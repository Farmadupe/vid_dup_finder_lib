for OPACITY in 0.1 0.2 0.3 0.4 0.5 0.6 0.7 0.8 0.9
do
    ffmpeg -y -i "orig.mp4" -crf 32 -vf "drawtext=text='watermark':x=10:y=H-th-10:fontsize=30:fontcolor=red@$OPACITY" dog.watermark_$OPACITY.mp4
done
