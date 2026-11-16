# test, plain build, check build of the tq worktree (run under the lock)
set -e
cd <scratchpad>/wt/tq
B=C:/Users/<user>/AppData/Local/Temp/snekb/tq
TC=stable-x86_64-pc-windows-msvc
export RUSTFLAGS="-Ctarget-cpu=native"
if [ "$1" = test ]; then
  cargo +$TC test --release --target-dir $B/test 2>&1 | grep -E "^error|-->|test |panicked|result" | head -40
fi
cargo +$TC build --release --target-dir $B/plain 2>&1 | grep -E "^error|-->|Finished" | head
cp $B/plain/release/snek.exe ../tq-$2.exe
cargo +$TC build --release --features tqcheck --target-dir $B/chk 2>&1 | grep -E "^error|-->|Finished" | head
cp $B/chk/release/snek.exe ../tqchk-$2.exe
sha256sum ../tq-$2.exe ../tqchk-$2.exe | cut -c1-12
