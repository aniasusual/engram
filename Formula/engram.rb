class Engram < Formula
  desc "Persistent codebase intelligence daemon for coding agents"
  homepage "https://github.com/aniasusual/engram"
  license "MIT"
  head "https://github.com/aniasusual/engram.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    # Verify binary runs and shows help
    assert_match "codebase intelligence", shell_output("#{bin}/engram --help")

    # Verify init creates database
    mkdir "test_repo"
    cd "test_repo" do
      system bin/"engram", "init"
      assert_predicate Pathname.new(".engram/engram.db"), :exist?
    end
  end
end
