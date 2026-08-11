class Arxiv < Formula
  desc "Fast, minimal arXiv CLI for humans and agents"
  homepage "https://github.com/ishaanko/arxiv-cli"
  version "0.1.0"
  license "MIT"

  if OS.mac? && Hardware::CPU.arm?
    url "https://github.com/ishaanko/arxiv-cli/releases/download/v0.1.0/arxiv-0.1.0-arm64-darwin.tar.gz"
    sha256 "afdddef9edd901a752f27a8444e999bf19045bc5a824ee4e873a71f0381aa916"

    def install
      bin.install "arxiv"
    end
  else
    url "https://github.com/ishaanko/arxiv-cli/archive/refs/tags/v0.1.0.tar.gz"
    sha256 "12e16bf2ab18c06fee26999eca0631563e17eb1b162355cb3e57aadca438c3af"

    depends_on "rust" => :build

    def install
      system "cargo", "install", *std_cargo_args
    end
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/arxiv --version")
  end
end
