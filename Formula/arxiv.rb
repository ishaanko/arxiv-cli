# Source-build formula for public distribution. After pushing this repo to
# GitHub and tagging v0.1.0, update url/sha256 and copy this file into a
# github.com/ishaanko/homebrew-arxiv tap repository.
class Arxiv < Formula
  desc "Fast, minimal arXiv CLI for humans and agents"
  homepage "https://github.com/ishaanko/arxiv-cli"
  url "https://github.com/ishaanko/arxiv-cli/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "REPLACE_WITH_RELEASE_TARBALL_SHA256"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/arxiv --version")
  end
end
