#!/bin/bash
# Writes the sources of the pak probe into an addon directory, see README.md.
set -e
cd "$1"

mkdir -p Configs/Beta Scripts/Game/Deep/Nested/Dir

printf 'Alpha {\n}\n' > Configs/alpha.conf
printf 'Beta {\n}\n' > Configs/Beta/beta.conf
: > Configs/empty.conf
printf 'Zeta {\n}\n' > Configs/Zeta.conf
printf 'Under {\n}\n' > Configs/_under.conf
printf 'Unicode {\n}\n' > Configs/ünïcode.conf
for i in $(seq 1 3000); do echo "// line $i padding padding padding padding padding"; done \
    > Scripts/Game/Deep/Nested/Dir/PP_Big.c
printf 'class PP_Tiny {}\n' > Scripts/Game/Deep/PP_Tiny.c
printf 'class PP_Hello\n{\n\tvoid Hi() { Print("hello"); }\n}\n' > Scripts/Game/PP_Hello.c

touch -d '2026-01-01 00:00:00 UTC' Configs/*.conf Configs/Beta/beta.conf
touch -d '2063-12-31 23:59:58 UTC' Scripts/Game/Deep/Nested/Dir/PP_Big.c
touch -d '2001-02-03 04:05:06 UTC' Scripts/Game/Deep/PP_Tiny.c
touch -d '2026-10-29 20:51:36 UTC' Scripts/Game/PP_Hello.c
