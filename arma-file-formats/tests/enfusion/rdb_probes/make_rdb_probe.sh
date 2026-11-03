#!/bin/bash
# Builds the RdbProbe addon in $1 (e.g. "<My Games>/ArmaReforgerWorkbench/addons/RdbProbe"), see
# README.md. Workbench then writes its resourceDatabase.rdb when it loads the addon.
set -e
R="$1"
rm -rf "$R"
mkdir -p "$R/Configs/Plain" "$R/Configs/Registered" "$R/Language"

cat > "$R/RdbProbe.gproj" <<'GPROJ'
GameProject {
 ID "RdbProbe"
 GUID "6B1D2C3E4F5A6978"
 TITLE "RdbProbe"
 Dependencies {
  "58D0FB3206B6F859"
 }
 Configurations {
  GameProjectConfig PC {
  }
  GameProjectConfig HEADLESS {
  }
 }
}
GPROJ

# conf <name> <guid> [tags line]: a .conf with a meta, the meta's Tags property if given.
conf() {
  printf 'Probe {\n}\n' > "$R/Configs/$1.conf"
  {
    printf 'MetaFileClass {\n Name "{%s}Configs/%s.conf"\n' "$2" "$1"
    if [ -n "$3" ]; then printf ' Tags %s\n' "$3"; fi
    printf ' Configurations {\n  CONFResourceClass PC {\n  }\n }\n}'
  } > "$R/Configs/$1.conf.meta"
}
conf a A000000000000001 '"tagA"'
conf b A000000000000002 '"tagB"'
conf ab A000000000000003 '"tagA tagB"'
conf c A000000000000004 '"tagC"'
conf cba A000000000000005 '"tagC tagB tagA"'
conf d A000000000000006 '"tagD" "tagE"'
conf none A000000000000007
conf Upper A000000000000008
conf _under A000000000000009
conf "ünïcode" A00000000000000A
printf 'Probe {\n}\n' > "$R/Configs/nometa.conf"

# A folder registered with its own meta, next to the folder.
printf 'MetaFileClass {\n Name "{B000000000000001}Configs/Registered"\n}' > "$R/Configs/Registered.meta"
printf 'Probe {\n}\n' > "$R/Configs/Registered/inner.conf"

printf 'StringTableSource {\n}\n' > "$R/Language/strings.st"
printf 'MetaFileClass {\n Name "{C000000000000001}Language/strings.st"\n Configurations {\n  StringTableResourceClass PC {\n  }\n }\n}' > "$R/Language/strings.st.meta"

# Fixed modification times (UTC) for the timestamps in the database.
find "$R" -type f -exec touch -d "2026-01-02 03:04:05 UTC" {} +
touch -d "2026-12-31 23:59:58 UTC" "$R/Configs/a.conf.meta"
