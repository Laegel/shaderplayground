cargo ndk -t arm64-v8a -o android/app/src/main/jniLibs build --release --no-default-features
cd android
gradle wrapper --gradle-version 8.5
./gradlew assembleDebug
