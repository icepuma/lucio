class Lucio < Formula
  desc "Clone Vivaldi profiles into isolated settings and extensions templates"
  homepage "https://github.com/icepuma/lucio"
  version "0.2.2"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/icepuma/lucio/releases/download/v0.2.2/lucio-v0.2.2-aarch64-apple-darwin.tar.gz"
      sha256 "494fa9c700279b66ca322c26bd52d6e8f2a17a3b99bf2008c6fd7e034fdff343"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/icepuma/lucio/releases/download/v0.2.2/lucio-v0.2.2-aarch64-unknown-linux-musl.tar.gz"
      sha256 "f9d3bc8fb72bd858d2de03f49b4e8b2dc3c5b7e6edc0d7307b1d982a1734aa06"
    end

    on_intel do
      url "https://github.com/icepuma/lucio/releases/download/v0.2.2/lucio-v0.2.2-x86_64-unknown-linux-musl.tar.gz"
      sha256 "b94ca79890231c0c2b137230a2b54c0824ab81ba5fdcaf2fe517f4ba4868ee28"
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
