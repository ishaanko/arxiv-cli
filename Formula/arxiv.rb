class Arxiv < Formula
  desc "Fast, minimal arXiv CLI for humans and agents"
  homepage "https://github.com/ishaanko/arxiv-cli"
  version "0.1.1"
  license "MIT"

  if OS.mac? && Hardware::CPU.arm?
    url "https://github.com/ishaanko/arxiv-cli/releases/download/v0.1.1/arxiv-0.1.1-arm64-darwin.tar.gz"
    sha256 "394212d99786bdf4e2d4df7dc54dc3f58ccb73d9b4de74da3f01370200bae554"

    def install
      bin.install "arxiv"
    end
  else
    url "https://github.com/ishaanko/arxiv-cli/archive/refs/tags/v0.1.1.tar.gz"
    sha256 "0c9396bded0801e1e7bd1a9d220dc5c03b9375764b932dcb0bfb660a52c3921c"

    depends_on "rust" => :build

    def install
      system "cargo", "install", *std_cargo_args
    end
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/arxiv --version")
  end
end
