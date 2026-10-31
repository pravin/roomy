# Homebrew cask for the desktop app. Lives in pravin/homebrew-tap as
# Casks/roomy.rb, so people can run `brew install --cask pravin/tap/roomy`.
# The release workflow builds one universal .dmg for Intel and Apple Silicon.
cask "roomy" do
  version "1.0.0"
  sha256 "REPLACE_WITH_SHA256_OF_THE_DMG"

  url "https://github.com/pravin/roomy/releases/download/v#{version}/Roomy_#{version}_universal.dmg"
  name "Roomy"
  desc "See what's filling your disk and clear out developer junk"
  homepage "https://github.com/pravin/roomy"

  livecheck do
    url :url
    strategy :github_latest
  end

  # Roomy updates itself, so Homebrew shouldn't fight it.
  auto_updates true
  depends_on macos: ">= :big_sur"

  app "Roomy.app"

  zap trash: [
    "~/Library/Application Support/io.github.pravin.roomy",
    "~/Library/Caches/io.github.pravin.roomy",
    "~/Library/WebKit/io.github.pravin.roomy",
  ]
end
