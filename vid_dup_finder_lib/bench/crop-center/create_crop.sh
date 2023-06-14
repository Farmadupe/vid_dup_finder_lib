for CROP in 0.1 0.15 0.2 0.25 0.3 0.35 0.4 0.45 0.5 0.55 0.6 0.65 0.7 0.75 0.8 0.85 0.9 0.95
do
    INVCROP=$(echo "scale=5; ((1.0 - $CROP) * 0.5)" | bc)
    ffmpeg -y -i "orig.mp4" -crf 32 -vf "crop=iw*$CROP:ih*$CROP:iw*$INVCROP:ih*$INVCROP" dog.crop_$CROP.mp4
done
