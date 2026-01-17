#!/bin/sh
APP_ID=io.github.noobping.listenmoe
BETA_ID=$APP_ID.beta
OUTDIR="${1:-AppDir/usr/share/locale}"

for f in po/*.po; do
    lang=$(basename "$f" .po)
    mkdir -p "$OUTDIR/$lang/LC_MESSAGES"
    msgfmt "$f" -o "$OUTDIR/$lang/LC_MESSAGES/$APP_ID.mo"
    cp "$OUTDIR/$lang/LC_MESSAGES/$APP_ID.mo" "$OUTDIR/$lang/LC_MESSAGES/$BETA_ID.mo"
done
