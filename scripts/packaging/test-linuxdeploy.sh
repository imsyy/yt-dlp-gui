#!/usr/bin/env bash
set -euo pipefail

# Verify the real bundler, including the nested deployment mode used by GTK.
linuxdeploy="$(realpath "${1:?Pass the linuxdeploy AppImage path}")"
work_dir="$(mktemp -d)"
trap 'rm -rf -- "$work_dir"' EXIT
chmod +x "$linuxdeploy"
mkdir -p "$work_dir/lib"

cat > "$work_dir/wayland.c" <<'EOF'
int wayland_fixture(void) { return 1; }
EOF
cat > "$work_dir/kept.c" <<'EOF'
int kept_fixture(void) { return 2; }
EOF
cat > "$work_dir/gtk.c" <<'EOF'
extern int wayland_fixture(void);
int gtk_fixture(void) { return wayland_fixture(); }
EOF
cat > "$work_dir/main.c" <<'EOF'
extern int wayland_fixture(void);
extern int kept_fixture(void);
int main(void) { return wayland_fixture() + kept_fixture() == 3 ? 0 : 1; }
EOF

cc -shared -fPIC "$work_dir/wayland.c" -Wl,-soname,libwayland-fixture.so.0 \
  -o "$work_dir/lib/libwayland-fixture.so.0"
ln -s libwayland-fixture.so.0 "$work_dir/lib/libwayland-fixture.so"
cc -shared -fPIC "$work_dir/kept.c" -Wl,-soname,libfixture-kept.so.0 \
  -o "$work_dir/lib/libfixture-kept.so.0"
ln -s libfixture-kept.so.0 "$work_dir/lib/libfixture-kept.so"
cc -shared -fPIC "$work_dir/gtk.c" -L"$work_dir/lib" -lwayland-fixture \
  -Wl,-rpath,"$work_dir/lib" -Wl,-soname,libgtk-fixture.so.0 \
  -o "$work_dir/lib/libgtk-fixture.so.0"
cc "$work_dir/main.c" -L"$work_dir/lib" -lwayland-fixture -lfixture-kept \
  -Wl,-rpath,"$work_dir/lib" -o "$work_dir/fixture"

export APPIMAGE_EXTRACT_AND_RUN=1 NO_STRIP=1 DISABLE_COPYRIGHT_FILES_DEPLOYMENT=1
deploy() {
  if ! "$linuxdeploy" --appimage-extract-and-run "$@" > "$work_dir/linuxdeploy.log" 2>&1; then
    cat "$work_dir/linuxdeploy.log" >&2
    return 1
  fi
}

# The control proves the fixture really has a deployable Wayland dependency.
unset LINUXDEPLOY_EXCLUDED_LIBRARIES
deploy --appdir "$work_dir/control" --executable "$work_dir/fixture"
test -f "$work_dir/control/usr/lib/libwayland-fixture.so.0"

export LINUXDEPLOY_EXCLUDED_LIBRARIES='libwayland-*.so*'
deploy --appdir "$work_dir/excluded" --executable "$work_dir/fixture"
# GTK's plugin explicitly deploys additional GTK libraries in plugin mode.
# Their transitive dependencies must continue to respect the exclusion.
LINUXDEPLOY_PLUGIN_MODE=1 deploy --appdir "$work_dir/excluded" \
  --library "$work_dir/lib/libgtk-fixture.so.0"
test -f "$work_dir/excluded/usr/lib/libfixture-kept.so.0"
test -f "$work_dir/excluded/usr/lib/libgtk-fixture.so.0"
if find "$work_dir/excluded" -name 'libwayland-*.so*' -print -quit | grep -q .; then
  echo 'Wayland libraries were bundled despite the exclusion' >&2
  exit 1
fi
echo 'linuxdeploy Wayland exclusion and GTK plugin-mode fixture passed'
