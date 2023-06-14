#!/bin/sh

#some folders
SRC_DIR=src_vids # for holding source videos -- will be deleted at the end of this script.
PROC_DIR=vids # procseed videos (truncated to 45 seconds)

#clean up outputs from previous execution
#rm -r $SRC_DIR
#rm -r $PROC_DIR

# Download two videos (with CC licenses) from youtube in multiple 
# different formats (all in low quality to avoid polluting the repository
# with large amounts of incompressible data)
# Then truncate them all to one minute long.
DOG_SRC="https://www.youtube.com/watch?v=xf0Mi3kWKhY"
CAT_SRC="https://www.youtube.com/watch?v=Mu7aPLc0Lq4"

get_src_vid() {
  URL=$1
  NICE_NAME=$2
  i=1

  youtube-dl -f 160 $URL --output $SRC_DIR/$NICE_NAME.$((i++)).mp4
  youtube-dl -f 394 $URL --output $SRC_DIR/$NICE_NAME.$((i++)).mp4
  youtube-dl -f 278 $URL --output $SRC_DIR/$NICE_NAME.$((i++)).webm

}

truncate_src_vids() {
  for F in $SRC_DIR/*
  do
    echo $(basename -- $F)
    mkdir -p $PROC_DIR
    ffmpeg -y -i $F -t 45 -c copy $PROC_DIR/$(basename -- $F)
  done
}

get_src_vid $DOG_SRC dog
get_src_vid $CAT_SRC cat

truncate_src_vids

#remove source videos
#rm -rf $SRC_DIR