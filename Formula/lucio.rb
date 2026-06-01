class Lucio < Formula
  desc "Clone Vivaldi profiles into isolated settings and extensions templates"
  homepage "https://github.com/icepuma/lucio"
  version "0.1.1"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/icepuma/lucio/releases/download/v0.1.1/lucio-v0.1.1-aarch64-apple-darwin.tar.gz"
      sha256 "04e5ac1e74cc7760669a0e653b7e6254d23ecf3ac265f34b61e9065cb039d844"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/icepuma/lucio/releases/download/v0.1.1/lucio-v0.1.1-aarch64-unknown-linux-musl.tar.gz"
      sha256 "b8935438c731fa05f9d54131e10244c54f6bf42f0e277f81472f2fa810493558"
    end

    on_intel do
      url "https://github.com/icepuma/lucio/releases/download/v0.1.1/lucio-v0.1.1-x86_64-unknown-linux-musl.tar.gz"
      sha256 "c20770cd8599342d304814a6621424da6228114f4cebdbfff43c2bfd3f8d9bab"
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
