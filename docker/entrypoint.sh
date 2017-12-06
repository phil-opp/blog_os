#!/bin/sh
USER_NAME=rustos
USER_UID=${LOCAL_UID:-9001}
USER_GID=${LOCAL_GID:-9001}

groupadd --non-unique --gid $USER_GID $USER_NAME
useradd --non-unique --create-home --uid $USER_UID --gid $USER_GID $USER_NAME

export HOME=/home/$USER_NAME

TESTFILE=$RUSTUP_HOME/settings.toml
CACHED_UID=$(stat -c "%u" $TESTFILE)
CACHED_GID=$(stat -c "%g" $TESTFILE)

if [ $CACHED_UID != $USER_UID  ] || [ $USER_GID != $CACHED_GID  ]; then
    chown $USER_UID:$USER_GID -R $CARGO_HOME $RUSTUP_HOME
fi

exec gosu $USER_NAME "$@"
