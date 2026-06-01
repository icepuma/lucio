class Lucio < Formula
  desc "Clone Vivaldi profiles into isolated settings and extensions templates"
  homepage "https://github.com/icepuma/lucio"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/icepuma/lucio/releases/download/v0.1.0/lucio-v0.1.0-aarch64-apple-darwin.tar.gz"
      sha256 "a1b726adbfb2bb2e86c7f2c670bd8808183282b55bc6b06505a90fca6b6d97d5"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/icepuma/lucio/releases/download/v0.1.0/lucio-v0.1.0-aarch64-unknown-linux-musl.tar.gz"
      sha256 "4d80f38503b70b985a146fd3e8b5fc4a7339e4f816c73e47de47bcd805d69400"
    end

    on_intel do
      url "https://github.com/icepuma/lucio/releases/download/v0.1.0/lucio-v0.1.0-x86_64-unknown-linux-musl.tar.gz"
      sha256 "587f1560ae87ed458d67c429faffd3db84dd45bbd007ff2ad287cb01b98bc5e9"
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
