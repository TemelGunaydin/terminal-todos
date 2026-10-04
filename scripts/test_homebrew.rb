# Run with Homebrew's Ruby libraries, without installing into the system Cellar:
# HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_DEVELOPER=1 brew ruby scripts/test_homebrew.rb
require "digest"
require "fileutils"
require "extend/ENV"
require "formulary"
require "formula_assertions"
require "tmpdir"

# brew ruby does not activate the build environment as brew install does.
ENV.activate_extensions!(env: "std")

root = Pathname.new(__dir__).parent
formula = Formulary.factory(root/"homebrew/Formula/todo.rb")
archive = root/"dist/terminal-todos-#{formula.version}.tar.gz"
expected = formula.stable.checksum.to_s
actual = Digest::SHA256.file(archive).hexdigest
raise "Archive checksum differs from the formula" unless actual == expected

# Preserve the existing toolchain/cache while isolating runtime HOME and all
# installation/log/build destinations. Nothing is linked into Homebrew's prefix.
cargo_home = ENV.fetch("CARGO_HOME", "#{Dir.home}/.cargo")
rustup_home = ENV.fetch("RUSTUP_HOME", "#{Dir.home}/.rustup")
# Homebrew deliberately removes user toolchains from PATH for developer commands.
path = ENV.fetch("HOMEBREW_PATH", ENV.fetch("PATH"))
Dir.mktmpdir("terminal-todos-homebrew-") do |temporary|
  directory = Pathname.new(temporary)
  prefix = directory/"prefix"
  source = directory/"terminal-todos-#{formula.version}"
  home = directory/"home"
  test_home = directory/"test"
  [home, test_home].each(&:mkpath)
  raise "Could not extract release archive" unless Kernel.system("tar", "-xzf", archive.to_s, "-C", directory.to_s)

  formula.define_singleton_method(:prefix) { |*_args| prefix }
  formula.define_singleton_method(:logs) { directory/"logs" }
  formula.define_singleton_method(:buildpath) { source }
  formula.extend(Homebrew::Assertions)
  formula.instance_variable_set(:@testpath, test_home)

  with_env("HOME" => home.to_s, "PATH" => path, "CARGO_HOME" => cargo_home, "RUSTUP_HOME" => rustup_home,
           "CARGO_TARGET_DIR" => (directory/"target").to_s) do
    Dir.chdir(source) { formula.install }
    Dir.chdir(test_home) { formula.test }
  end
  raise "The formula did not install todo" unless (prefix/"bin/todo").executable?
  puts "PASS: real Homebrew formula install and test methods, isolated prefix, archive checksum verified"
end
