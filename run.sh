#! /bin/bash

debug_mode=false
target=""
verbose=false
usage() {
    echo "Usage: $0 [-v] [-d] [-f feature] <target> [args]"
    echo "Example: ./run.sh -f 'd2000' can_server '0x43', which will compile and run example can_server with args=0x43 in sudo privilege"
    exit 1
}

while getopts ":vdf:" opt; do
  case ${opt} in
    v )
      verbose=true
      ;;
    d )
      debug_mode=true
      ;;
    f )
      feature=$OPTARG
      ;;
    \? )
      usage
      ;;
    : )
      echo "Option -$OPTARG requires an argument." >&2
      usage
      ;;
  esac
done
shift $((OPTIND -1))

if [ $# -eq 1 ]; then
    target=$1
elif [ $# -eq 2 ]; then
    target=$1
    ARGS=$2
else
    usage
fi

echo $feature

if $debug_mode; then
    run_path="debug"
    build_arg=""
else
    run_path="release"
    build_arg="--release"
fi

if [ -z $feature ]; then
    attach_feature=""
else
    attach_feature="--features \"$feature\""
fi
if $verbose; then
    cargo build --example $target $attach_feature $build_arg 
else
    cargo build --example $target $attach_feature $build_arg 2>/dev/null
fi 
if [ $? -ne 0 ]; then
    echo "build failed"
    exit 1
fi

rust_log=${RUST_LOG}
cmd="sudo RUST_LOG=$rust_log target/$run_path/examples/$target $ARGS"
eval $cmd