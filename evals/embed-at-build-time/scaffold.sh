#!/bin/sh
# Sets up this case's working copy (see ../_scaffold/scaffold.sh).
here=$(cd "$(dirname "$0")" && pwd)
exec sh "$here/../_scaffold/scaffold.sh" "$here"
