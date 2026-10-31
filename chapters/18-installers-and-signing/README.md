# Chapter 18 - Installers and signing

[< Chapter 17: Settings and polish](../17-settings-and-polish/README.md)

So far Roomy has only ever run from a terminal, with `npm run tauri dev`. Nobody else is going to do that. People expect a `.dmg` they can drag to Applications, an installer they can double-click on Windows, or a package their Linux distribution understands.

This chapter is short on code and long on explanation. We make installers for all three operating systems with one command, and then face the less fun part: why macOS and Windows will warn people about them, and what it costs to make the warnings go away.

In this chapter you will

- build release installers with `npm run tauri build`,
- find out what each operating system gets,
- fill in the bundle settings in `tauri.conf.json`,
- learn why the app's identifier matters,
- make the program small with Cargo's release profile,
- learn what code signing is, and what it costs on each platform.

## Before we begin

The only change from chapter 17 is in `app/src-tauri/tauri.conf.json`, where the `bundle` section has grown. The release profile in the top `Cargo.toml` has been there since chapter 4, but this is the chapter where it matters.

## Building an installer

```text
cd chapters/18-installers-and-signing/app
npm install
npm run tauri build
```

This takes a few minutes the first time. It builds the web page with Vite (that's the `beforeBuildCommand`), builds Roomy in release mode, and then wraps it up. The installers land in the workspace's target folder, under `target/release/bundle`. Which ones you get depends on the computer you build on. Tauri can't make a Windows installer on a Mac, or the other way round. (In the next chapter, GitHub builds all of them for us.)

| Built on | You get | What it is |
| --- | --- | --- |
| macOS | `macos/Roomy.app` | The app itself. Drag it to Applications. |
| | `dmg/Roomy_1.0.0_aarch64.dmg` | A disk image with the app in it, the usual way Mac apps are handed out. |
| Windows | `msi/Roomy_1.0.0_x64_en-US.msi` | A Windows Installer package. IT departments like these. |
| | `nsis/Roomy_1.0.0_x64-setup.exe` | An ordinary setup program. Most people will use this one. |
| Linux | `deb/Roomy_1.0.0_amd64.deb` | For Debian, Ubuntu and friends. |
| | `rpm/Roomy-1.0.0-1.x86_64.rpm` | For Fedora, openSUSE and friends. |
| | `appimage/Roomy_1.0.0_amd64.AppImage` | One file that runs on almost any Linux. Mark it executable and run it. |

Open the app you just built and have a look at the menu bar from chapter 17. With `npm run tauri dev`, macOS was running a bare program called `roomy-app`, and it used that name in the Dock and as the bold menu name. Now it's a real app bundle with `productName` "Roomy", so Roomy is what you see everywhere. **Roomy, About Roomy** (**Help, About Roomy** on Windows and Linux) shows the version from this file.

`"targets": "all"` in `tauri.conf.json` means "every kind this computer can make". You can list them instead, as in `["dmg", "nsis"]`.

On Linux you'll need the same packages you needed in chapter 4, plus `patchelf` for the AppImage. On Windows Tauri downloads the tools it needs to make the MSI the first time.

## Breaking it up

### The bundle section

```text
27    "bundle": {
28      "active": true,
29      "targets": "all",
30      "category": "Utility",
31      "shortDescription": "See what's filling your disk",
32      "longDescription": "Roomy scans a disk or folder in seconds, shows you what's taking up the space as a treemap, and finds developer junk like node_modules and build folders that you can safely throw away.",
33      "copyright": "© 2026 Pravin Paratey",
34      "icon": [
...
41      "macOS": {
42        "minimumSystemVersion": "11.0"
43      },
44      "linux": {
45        "deb": {
46          "depends": []
47        }
48      }
49    }
```

Most of this is information the installers pass on. `category` [line 30] decides where Roomy appears in the Linux applications menu and in the macOS Finder's "Kind" column. The descriptions [lines 31 and 32] show up in Linux package managers and the Windows installer. `copyright` [line 33] goes into the Mac app's Get Info window and the Windows file properties.

`minimumSystemVersion` [line 42] says Roomy needs macOS 11, Big Sur, from 2020. Older versions of macOS have older versions of the web view that Tauri uses, and some of the things our stylesheet does won't work in them. It's better for the installer to say so than for the window to come out wrong.

`depends` [line 46] is for packages the `.deb` should pull in. Tauri already adds the web view library it needs, so we don't need anything extra. I've left it in so you know where it goes.

### The identifier

```text
 5    "identifier": "io.github.pravin.roomy",
```

Every app needs a name that no other app in the world has. Apple calls it a *bundle identifier*. Windows uses it to keep apps apart, and Tauri uses it for the settings folder from the last chapter. The convention is to take a domain name you own, reverse it, and add the app's name: `com.apple.finder`, `org.mozilla.firefox`.

I don't own `roomy.app`. If I made up `app.roomy.desktop` and someone who does own that domain made an app later, we'd clash. Worse, some app stores check. Flathub, which we meet in chapter 20, only accepts an app whose identifier is a domain you can prove you control. A GitHub account gives you one for free: `pravin.github.io` is mine, so `io.github.pravin.roomy` is too.

Choose this carefully, because it's hard to change later. The settings folder is named after it, so everybody who already has Roomy would lose their settings.

### Small, by request

```text
22  [profile.release]
23  lto = true
24  codegen-units = 1
25  strip = true
```

A *profile* is a set of choices Cargo makes when it builds. `cargo build` uses the `dev` profile, which compiles fast and makes slow, large programs full of debugging information. `cargo build --release`, and `tauri build`, use `release`. We've changed three of its choices.

- `lto = true` is *link time optimisation*. Normally Rust optimises each crate on its own. With LTO it looks at the whole program at once, across all our dependencies, and can throw away code that's never called and inline functions from one crate into another. Smaller and usually faster, but slower to build.
- `codegen-units = 1` stops Rust splitting each crate into pieces to compile them in parallel. Again, slower to build, but the optimiser sees more at once.
- `strip = true` removes the symbols, the names of all the functions, which are only there for debuggers.

The result is a Roomy that's a few megabytes. Expect something in the region of 5 to 10 MB for the installer, depending on the platform. An Electron app, which ships its own copy of Chromium, starts at well over 100 MB. That's the main reason to use Tauri: Roomy uses the web view your operating system already has (WebKit on macOS and Linux, Edge's WebView2 on Windows) instead of bringing its own.

## Code signing

Now the bad news. Build Roomy on your Mac, copy the `.dmg` to a friend's Mac and open it, and macOS will refuse: *"Roomy" can't be opened because Apple cannot check it for malicious software.* On Windows, a blue box says *Windows protected your PC* and hides the Run button behind "More info".

Neither is a bug. They are your operating system being careful about programs downloaded from the internet. To get past them you have to *sign* the program: attach a cryptographic signature that says who made it and proves nobody has changed it since. The operating system checks the signature against a certificate, and it trusts the certificate because someone it trusts issued it, after checking who you are. That checking is what costs money.

### macOS

Apple's macOS security check is called **Gatekeeper**. To satisfy it you need

1. a **Developer ID** certificate, which you get by joining the **Apple Developer Program**. It costs $99 a year, and Apple checks your identity when you join.
2. **notarisation**, where you upload each build to Apple, a robot scans it for malware, and Apple gives it a ticket to say so. It takes a few minutes.

Tauri does both for you during `tauri build` if you set the right environment variables: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY` for signing, and `APPLE_ID`, `APPLE_PASSWORD` (an app-specific password, not your real one) and `APPLE_TEAM_ID` for notarisation. The release workflow in the next chapter passes them on if you've set them up.

It used to be possible to ship an unsigned Mac app and tell people to right-click it and choose Open. That still works, more or less, but it's getting harder every year, and it now matters for distribution too. Homebrew, the most popular way for Mac developers to install apps, has started disabling casks that fail Gatekeeper's check. If you want Roomy on Homebrew, which we do in chapter 20, you need to pay the $99.

### Windows

Windows uses a reputation system called **SmartScreen**. An unsigned program from the internet gets the warning. A signed one gets it too at first, until enough people have run it without anything bad happening. Signing doesn't stop the warning straight away, but it starts the clock and it puts your name on the program instead of "Unknown publisher".

Certificates for Windows used to cost hundreds a year and come on a USB key. There are two cheaper ways now.

- **SignPath Foundation** signs open source projects for free. You apply, they check your project builds from its public source, and your GitHub release workflow sends builds to them to be signed.
- **Azure Trusted Signing** from Microsoft costs about $10 a month. Microsoft checks who you are, and then signs from the cloud.

Either works with Tauri. Its documentation has a page for each.

### Linux

Nothing. Linux doesn't check signatures on programs you download yourself. Packages from your distribution's own repositories are signed by the distribution, and Flathub signs everything it builds. Neither needs anything from you.

### What I'd do

If you're just starting, ship unsigned and say so in your README. People who download developer tools are used to right-clicking Open. When you want Homebrew, or people who aren't developers, pay Apple. Apply to SignPath for Windows. I'd rather tell you that up front than have you find out from your first bug report.

## Adding functionality

Make the Mac build work on both Intel and Apple Silicon Macs in one file, a *universal* binary.

1. `rustup target add x86_64-apple-darwin aarch64-apple-darwin`
2. `npm run tauri build -- --target universal-apple-darwin`

Rust builds Roomy twice, once for each kind of processor, and Tauri glues them together. The `.dmg` ends up in `target/universal-apple-darwin/release/bundle/dmg`.

## Exercise

Build only the `.deb` and the AppImage on Linux, or only the NSIS installer on Windows, without changing `tauri.conf.json`.

*Hint: `npm run tauri build -- --help` lists an option for picking bundles.*

## That's it

We can make installers, and we know what it takes to make them trusted. But each new version means building on three computers and uploading by hand, and everybody who has Roomy has to come back and download it again. In the next chapter GitHub builds every installer for us, and Roomy learns to update itself.

[Chapter 19: Auto-update](../19-auto-update/README.md)
