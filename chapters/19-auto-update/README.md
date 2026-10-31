# Chapter 19 - Auto-update

[< Chapter 18: Installers and signing](../18-installers-and-signing/README.md)

Software is never finished. You'll fix a bug next week, someone will ask for a feature the week after, and every person who already has Roomy will be stuck on the old one unless they happen to come back to the website. Very few people come back to the website.

So in this chapter Roomy learns to check for a newer version when it starts, and to install it with one click. And since a release now means building installers for three operating systems, we hand that job to GitHub as well. By the end, releasing a new version is two commands.

In this chapter you will

- add the updater and process plugins,
- make a key pair for signing updates, and keep the private half secret,
- show an "update available" banner,
- build every installer on GitHub Actions when you push a version tag,
- set up the secrets the build needs,
- cut your first release.

## Before we begin

New file: `app/src/update.ts`. `main.rs` gets two more plugins, the capability file two more permissions, and `tauri.conf.json` a `plugins` section. `index.html` gets an empty banner at the top, and `main.ts` calls `checkForUpdate()` once at start-up. The menu bar from chapter 17 gets a **Check for Updates…** item too, in the Roomy menu on a Mac and the Help menu on Windows and Linux.

The release workflow lives at the top of the repository, in `.github/workflows/release.yml`, because that's the only place GitHub looks for workflows. It builds chapter 20, the last chapter, which has the same code as this one.

## How updates work

The idea is simple. With each release we put a small file on GitHub called `latest.json`. It says what the newest version is, and where to download it for each operating system. When Roomy starts, it fetches that file. If the version in it is newer than its own, it offers to download the new one and install it over itself.

The danger is just as simple. A program that downloads programs and runs them is exactly what an attacker would love to take over. If someone could swap `latest.json` or the download for one of their own, every copy of Roomy would install their program for them. So every update is *signed*, the same idea as code signing in the last chapter, but with a key that belongs to us, not Apple or Microsoft. Roomy has the public half of the key built into it. Before it installs anything, it checks the download's signature against that key. Only someone with the private half can make a signature that passes. So the private half must never leave your hands.

## Breaking it up

### The key pair

```text
npx tauri signer generate -w ~/.tauri/roomy.key
```

This asks for a password and writes two files: `~/.tauri/roomy.key`, the private key, and `~/.tauri/roomy.key.pub`, the public one. I keep them in my home folder, not in the repository, and the repository's `.gitignore` ignores `*.key` in case one ever wanders in. If the private key ends up on GitHub, anybody can sign updates as you. If you lose it, you can't sign updates at all, and everybody who has Roomy is stuck on their version forever, because their copy only trusts the old public key. Keep a backup somewhere safe, like your password manager.

The public key is not a secret. It goes into `tauri.conf.json`.

```text
51    "plugins": {
52      "updater": {
53        "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDRDM0VCQUIwNTAyQUM5Q0UKUldUT3lTcFFzTG8rVE5YZkQ3c3UrblAyd2JXcWxDdWdrY3NiM3dyUWlqNzFzbFYxaVNYRm5MR0IK",
54        "endpoints": [
55          "https://github.com/pravin/roomy/releases/latest/download/latest.json"
56        ]
57      }
58    }
```

That's mine [line 53]. If you're making your own app, put yours there instead, the contents of the `.pub` file. The endpoint [line 55] is a GitHub address with a useful habit: `releases/latest/download/` always points at the newest published release. So Roomy always fetches the newest `latest.json`, and we never have to change this address.

### Update files, but only in CI

```text
49      "createUpdaterArtifacts": false
```

With `createUpdaterArtifacts` turned on, `tauri build` makes signed update files next to the installers. To sign them it needs the private key, from the `TAURI_SIGNING_PRIVATE_KEY` environment variable, and stops with an error if it isn't there. That would break `npm run tauri build` on your own computer and on anybody else's who checks out the code. So it's off here, and the release workflow turns it on for its own build only. You'll see how below.

### The plugins

```text
27          .plugin(tauri_plugin_updater::Builder::new().build())
28          .plugin(tauri_plugin_process::init())
```

The updater plugin does the fetching, checking and installing. The process plugin can restart the app, so the new version starts when the old one is done. As always, the window can only use them if the capability file says so: `updater:default` lets it check and install, and `process:allow-restart` lets it restart Roomy, and nothing else the process plugin can do, like quitting.

### The banner

```text
10  // `asked` is true when someone chose Check for Updates from the menu, so
11  // they deserve an answer even when there's nothing new.
12  export async function checkForUpdate(asked = false) {
13    let update;
14    try {
15      update = await check();
16    } catch (error) {
17      // Offline, or no release yet. Try again next time.
18      if (asked) message(`Couldn't check for updates.\n\n${error}`, { title: "Roomy", kind: "warning" });
19      return;
20    }
21    if (!update) {
22      if (asked) message("You have the latest version of Roomy.", { title: "Roomy" });
23      return;
24    }
```

`check()` [line 15] fetches `latest.json` and compares versions. It gives back `null` if we're up to date [line 21], and throws if anything goes wrong. Things often go wrong in normal life: you're on a train, GitHub is having a bad day, or, before the first release, there's simply no file to fetch.

When Roomy checks by itself at start-up, none of that is worth bothering anybody about, so we give up quietly [line 19] and try again next time. But when someone picks **Check for Updates…** from the menu, they're waiting for an answer, and silence would look like the menu item is broken. That's what `asked` is for [line 12]. It's `false` unless we say otherwise (`asked = false` gives the parameter a default, so the start-up call can leave it out), and when it's `true` we say what happened: either that the check failed [line 18], or "You have the latest version of Roomy." [line 22].

```text
26    const banner = $("#update");
27    const button = h("button", {}, "Update and restart");
28    button.onclick = async () => {
29      button.disabled = true;
30      button.textContent = "Downloading…";
31      await update.downloadAndInstall();
32      await relaunch();
33    };
34    banner.replaceChildren(h("span", {}, `Roomy ${update.version} is out.`), button);
35    banner.hidden = false;
```

If there's a newer version, a green banner appears across the top. We don't update without asking. Someone might be halfway through tidying their disk. When they click, the plugin downloads, checks the signature, and installs [line 31]. Then we start again [line 32]. If the signature doesn't match, `downloadAndInstall` throws and nothing is installed.

### The menu item

`menu.rs` makes one more item, the same way as all the others from chapter 17.

```text
66      let updates = item("check-updates", "Check for Updates…", true, None)?;
```

It goes just under About Roomy in the Roomy menu on a Mac, and at the bottom of the Help menu on Windows and Linux, which is where people look for it on each. Clicking it sends `check-updates` to the page, and the `onMenu` switch in `main.ts` passes it on.

```text
140      case "check-updates":
141        checkForUpdate(true);
142        break;
```

That `true` is the `asked` from above.

## Building releases on GitHub

GitHub Actions runs jobs on GitHub's computers when something happens to your repository. They have Macs, Windows machines and Linux machines, so they can build all three kinds of installer at once. Here's the release workflow, in `.github/workflows/release.yml` at the top of the repository.

```text
 5  on:
 6    push:
 7      tags: ["v*"]
```

It runs when you push a tag that starts with `v`, like `v1.0.0` [line 7]. Not on every commit, just when you say "this is a release".

```text
15  jobs:
16    app:
17      strategy:
18        fail-fast: false
19        matrix:
20          include:
21            - os: macos-latest
22              args: --target universal-apple-darwin
23            - os: ubuntu-22.04
24              args: ""
25            - os: windows-latest
26              args: ""
27      runs-on: ${{ matrix.os }}
```

A *matrix* runs the same steps several times with different settings. Here it's three times, once per operating system [lines 21 to 26], all at the same time. The Mac build is universal [line 22], the trick from the last chapter's "Adding functionality", so one `.dmg` works on every Mac. Linux builds on Ubuntu 22.04, not the newest, because a program built against an older Linux runs on newer ones, but not the other way round. `fail-fast: false` [line 18] means a failure on one keeps the others going, so you can see all your problems at once.

The steps install the Linux libraries Tauri needs, Rust, Node and the npm packages, much as you did in chapter 4.

### Secrets

A workflow needs your private key, and the repository is public. GitHub's answer is **secrets**: values you type into the repository's **Settings, Secrets and variables, Actions** page. Workflows can use them, but nobody can read them back, and GitHub blanks them out of the logs. Set these:

| Secret | What it is |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | The contents of `~/.tauri/roomy.key` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Its password |
| `APPLE_CERTIFICATE` | Your Developer ID certificate, exported as a `.p12` and base64 encoded (optional) |
| `APPLE_CERTIFICATE_PASSWORD` | The `.p12`'s password (optional) |
| `APPLE_SIGNING_IDENTITY` | Like `Developer ID Application: Your Name (TEAMID)` (optional) |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | For notarisation. The password is an app-specific one from appleid.apple.com (optional) |

The Apple ones are optional, but there's a catch. If a secret isn't set, `${{ secrets.X }}` is an empty string, not nothing. Tauri sees `APPLE_CERTIFICATE` is set, tries to read an empty certificate, and fails. So one step passes on only the secrets that have something in them.

```text
63          run: |
64            for name in APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD APPLE_SIGNING_IDENTITY APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID; do
65              if [ -n "${!name}" ]; then
66                {
67                  echo "$name<<END_OF_SECRET"
68                  echo "${!name}"
69                  echo "END_OF_SECRET"
70                } >> "$GITHUB_ENV"
71              fi
72            done
```

`${!name}` is bash for "the variable whose name is in `name`". `-n` [line 65] means "isn't empty". Anything written to the `$GITHUB_ENV` file becomes an environment variable for the later steps. The `<<END_OF_SECRET` form [lines 67 to 69] allows values over several lines, which a certificate is. Without Apple secrets you still get a working `.dmg`, just an unsigned one.

### The build

```text
74        - uses: tauri-apps/tauri-action@v0
...
80          with:
81            projectPath: ${{ env.CHAPTER }}/app
82            tagName: ${{ github.ref_name }}
83            releaseName: Roomy ${{ github.ref_name }}
84            releaseBody: See the assets below to download Roomy for your computer.
85            releaseDraft: true
86            includeUpdaterJson: true
87            args: ${{ matrix.args }} --config {"bundle":{"createUpdaterArtifacts":true}}
```

`tauri-action` is made by the Tauri team. It runs `tauri build`, makes a GitHub release for the tag if there isn't one yet, and uploads the installers to it. `releaseDraft: true` [line 85] keeps the release hidden until you've had a look at it. `includeUpdaterJson: true` [line 86] makes it write `latest.json` too, with an entry for each operating system as its build finishes.

[Line 87] is where `createUpdaterArtifacts` gets turned on. `--config` takes a bit of JSON and lays it over `tauri.conf.json` for this build only. Here the private key is in the environment, so signing works.

### The command line tool

The second job, `cli`, waits for the first (`needs: app` [line 90]) and builds the `roomy` command for four targets: Linux, both kinds of Mac and Windows. It packs each into a `.tar.gz` or `.zip` named like `roomy-cli-1.0.0-aarch64-apple-darwin.tar.gz` and adds it to the same release with `gh release upload` [lines 129 and 132]. `--locked` [line 117] makes Cargo use exactly the versions in `Cargo.lock`, so the release is built from the same crates you tested with. Chapter 20 points package managers at these files.

### Cutting a release

Change the version in three places: `tauri.conf.json`, `app/package.json` and the `version` in the top `Cargo.toml`. The updater compares the version in `tauri.conf.json` against `latest.json`, so that one matters most. Commit, then

```text
git tag v1.0.0
git push --tags
```

Go to the repository's **Actions** tab and watch. About fifteen minutes later there's a draft release with a `.dmg`, an `.msi`, a setup `.exe`, a `.deb`, an `.rpm`, an AppImage, four command line archives, the signature files and `latest.json`. Open the draft, write a few lines about what changed, and click **Publish**. From that moment, every copy of Roomy that starts sees the banner.

The release workflow can't be tested on your own computer, and the first run of any workflow usually fails for some silly reason. Try it with a tag like `v0.9.0-test` first, check the draft, then delete it.

## Adding functionality

Show what's new in the banner. `update.body` holds the text you wrote when you published the release.

1. In `update.ts`, after the version, add `h("small", {}, update.body ?? "")`.
2. Keep it short, or make it a `title` on the banner instead so it shows when you point at it.

## Exercise

If the download fails, the button stays disabled and says "Downloading…" forever. Fix it so that it says what went wrong, and lets you try again.

*Hint: Wrap lines 23 and 24 in `try`/`catch`. In the `catch`, set the text, and set `disabled` back to `false`.*

## That's it

GitHub builds every installer whenever we tag a release, and Roomy keeps itself up to date. The last step is getting it in front of people. In the final chapter we put Roomy into Homebrew, winget, Scoop, the AUR, Flathub and crates.io, so it's one command away on every system.

[Chapter 20: Package managers](../20-package-managers/README.md)
