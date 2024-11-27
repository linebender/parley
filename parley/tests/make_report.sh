#!/bin/bash

if ! command -v image_diff_review 2>&1 >/dev/null
then
    echo "image_diff_review not found. Install it via 'cargo install image_diff_review'"
    exit 1
fi

image_diff_review --ignore-left-missing --left-title "Current test" --right-title "Snaphost" current snapshots report
