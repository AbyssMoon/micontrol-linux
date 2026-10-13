# source env.sh — локальные тулчейны проекта (ничего не ставится в систему)
_MC_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export RUSTUP_HOME="$_MC_ROOT/.toolchain/rustup"
export CARGO_HOME="$_MC_ROOT/.toolchain/cargo"
export QT_DIR="$(ls -d "$_MC_ROOT"/.toolchain/qt/6.*/gcc_64 2>/dev/null | head -1)"
export QMAKE="$QT_DIR/bin/qmake"
export PATH="$CARGO_HOME/bin:$QT_DIR/bin:$PATH"
export LD_LIBRARY_PATH="$QT_DIR/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export CXXFLAGS="-Wno-sfinae-incomplete${CXXFLAGS:+ $CXXFLAGS}"
# libGL.so (dev-симлинк) нужен только линковщику; сама библиотека — системная.
export RUSTFLAGS="-L native=$_MC_ROOT/.toolchain/linkfix${RUSTFLAGS:+ $RUSTFLAGS}"
