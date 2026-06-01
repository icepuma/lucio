class Lucio < Formula
  desc "Clone Vivaldi profiles into isolated settings and extensions templates"
  homepage "https://github.com/icepuma/lucio"
  version "0.2.1"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/icepuma/lucio/releases/download/v0.2.1/lucio-v0.2.1-aarch64-apple-darwin.tar.gz"
      sha256 "01c23d48e9f6c5e3762d44d7cc50c05615910cb3630ae571dc5a7b58ffbb1e39"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/icepuma/lucio/releases/download/v0.2.1/lucio-v0.2.1-aarch64-unknown-linux-musl.tar.gz"
      sha256 "220bd6a9ef1f68c1eb48d33f1b87c553fa74b3183f942caa038dbd4e6a1e18fa"
    end

    on_intel do
      url "https://github.com/icepuma/lucio/releases/download/v0.2.1/lucio-v0.2.1-x86_64-unknown-linux-musl.tar.gz"
      sha256 "d5a765e92cc8fe93f0add505c244fa7318b9adbc5acbd9a37c6186657ee14ebc"
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
