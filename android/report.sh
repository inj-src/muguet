#!/bin/sh
# muguet - android/report.sh
# Constantin Clerc (c22dev) - 0.1
#
# Everything needed to look into a problem on an Android device connected with adb, in one file.
# Run it while the game is frozen (it dumps its threads) or right after a crash.

P=ch.cclerc.muguet
D=/sdcard/Android/data/$P/files
OUT="${1:-$HOME/muguet-report.txt}"
PID="$(adb shell pidof $P | tr -d '\r')"
{
  echo "== device"
  for p in ro.product.manufacturer ro.product.model ro.soc.manufacturer ro.soc.model ro.build.version.release ro.build.version.sdk ro.hardware.vulkan; do
    echo "$p: $(adb shell getprop $p | tr -d '\r')"
  done
  adb shell cat /proc/meminfo | head -3
  echo "== gpu"
  adb shell cmd gpu vkjson 2>/dev/null | grep -a -E '"(deviceName|driverVersion|apiVersion|textureCompressionBC)"' | head -8
  echo "== how the app last ended"
  adb shell dumpsys activity exit-info $P | grep -a -E 'timestamp=|reason=' | head -12
  echo "== status (pid: ${PID:-not running})"
  adb shell cat $D/muguet-status.txt
  if [ -n "$PID" ]; then
    echo "== memory"
    adb shell dumpsys meminfo $P | head -40
    echo "== threads"
    adb shell debuggerd -b "$PID"
  fi
  echo "== crash buffer"
  adb logcat -d -b crash | grep -a -A40 "$P" | tail -120
  echo "== logcat (muguet)"
  adb logcat -d -s muguet | tail -200
  for f in muguet-previous.log muguet.log; do
    echo "== $f: $(adb shell wc -l $D/$f | tr -d '\r') (head)"
    adb shell head -n 150 $D/$f
    echo "== $f (tail)"
    adb shell tail -n 300 $D/$f
  done
  echo "== last_error.txt"; adb shell cat $D/last_error.txt
  echo "== config.json"; adb shell cat $D/config.json
} > "$OUT" 2>&1
echo "wrote $OUT ($(wc -l < "$OUT") lines)"
