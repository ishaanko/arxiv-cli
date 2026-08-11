# Source-build formula, mirrored in the github.com/ishaanko/homebrew-arxiv tap.
class Arxiv < Formula
  desc "Fast, minimal arXiv CLI for humans and agents"
  homepage "https://github.com/ishaanko/arxiv-cli"
  url "https://github.com/ishaanko/arxiv-cli/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "12e16bf2ab18c06fee26999eca0631563e17eb1b162355cb3e57aadca438c3af"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/arxiv --version")
  end
end
