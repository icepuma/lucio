class Lucio < Formula
  desc "Clone Vivaldi profiles into isolated settings and extensions templates"
  homepage "https://github.com/icepuma/lucio"
  version "0.3.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/icepuma/lucio/releases/download/v0.3.0/lucio-v0.3.0-aarch64-apple-darwin.tar.gz"
      sha256 "1d1a19cfb45dcbcacab736fdab505f149e83c9d9f9206e25beed494a60f48d0e"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/icepuma/lucio/releases/download/v0.3.0/lucio-v0.3.0-aarch64-unknown-linux-musl.tar.gz"
      sha256 "1aa0102b8b27599834c08858e16dfd733a75733521dcbaf619ceb583e417aabf"
    end

    on_intel do
      url "https://github.com/icepuma/lucio/releases/download/v0.3.0/lucio-v0.3.0-x86_64-unknown-linux-musl.tar.gz"
      sha256 "b6a625db7cf7e8430ce87fb26a7d2f344ff198aef07507a7c63987f43bc3f649"
    end
  end

  def install
    bin.install "lucio"
    doc.install "README.md"
    generate_completions_from_executable(bin/"lucio", "completions")
  end

  test do
    assert_match "lucio #{version}", shell_output("#{bin}/lucio --version")
  end
end
