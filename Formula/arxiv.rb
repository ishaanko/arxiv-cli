class Arxiv < Formula
  desc "Fast, minimal arXiv CLI for humans and agents"
  homepage "https://github.com/ishaanko/arxiv-cli"
  version "0.1.2"
  license "MIT"

  if OS.mac? && Hardware::CPU.arm?
    url "https://github.com/ishaanko/arxiv-cli/releases/download/v0.1.2/arxiv-0.1.2-arm64-darwin.tar.gz"
    sha256 "9626094ab7d5af98279c4dcd45968466a25f8509626b07e70eea8139b3b904e6"

    def install
      bin.install "arxiv"
    end
  else
    url "https://github.com/ishaanko/arxiv-cli/archive/refs/tags/v0.1.2.tar.gz"
    sha256 "0fd6232f6d58f80946b199cc886167b258d1d458a1b5b9489097c3cb95f1595e"

    depends_on "rust" => :build

    def install
      system "cargo", "install", *std_cargo_args
    end
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/arxiv --version")
  end
end
