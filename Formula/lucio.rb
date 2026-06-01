class Lucio < Formula
  desc "Clone Vivaldi profiles into isolated settings and extensions templates"
  homepage "https://github.com/icepuma/lucio"
  version "0.2.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/icepuma/lucio/releases/download/v0.2.0/lucio-v0.2.0-aarch64-apple-darwin.tar.gz"
      sha256 "e349529f309312ec35d6b4568dfeb3566ca0e6e438246e6b9545bc0adbee36cf"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/icepuma/lucio/releases/download/v0.2.0/lucio-v0.2.0-aarch64-unknown-linux-musl.tar.gz"
      sha256 "76dfc5e05028ce7ea7056a2e8baa7209457ba52b67242d993cdd888fdc74bdf1"
    end

    on_intel do
      url "https://github.com/icepuma/lucio/releases/download/v0.2.0/lucio-v0.2.0-x86_64-unknown-linux-musl.tar.gz"
      sha256 "b0a71f6d0a0adb371a462cf411babb50de59f2591f1dd2b06a2184695eea3c48"
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
