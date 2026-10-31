# Homebrew formula for the command line tool. Lives in pravin/homebrew-tap
# as Formula/roomy-cli.rb: `brew install pravin/tap/roomy-cli`.
# It builds from source, which is what Homebrew prefers for formulae.
class RoomyCli < Formula
  desc "Find out what's filling your disk, and clear out developer junk"
  homepage "https://github.com/pravin/roomy"
  url "https://github.com/pravin/roomy/archive/refs/tags/v1.0.0.tar.gz"
  sha256 "REPLACE_WITH_SHA256_OF_THE_SOURCE_TARBALL"
  license "Apache-2.0"

  depends_on "rust" => :build

  def install
    cd "chapters/20-package-managers" do
      system "cargo", "install", *std_cargo_args(path: "cli")
    end
  end

  test do
    (testpath/"folder/file.txt").write "hello"
    assert_match "1 files", shell_output("#{bin}/roomy #{testpath}/folder 2>&1")
  end
end
