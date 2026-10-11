// muguet - app/build.gradle.kts
// Constantin Clerc (c22dev) - 0.1

plugins {
    id("com.android.application")
}

android {
    namespace = "ch.cclerc.muguet"
    compileSdk = 35

    defaultConfig {
        applicationId = "ch.cclerc.muguet"
        // Vulkan 1.1 drivers and AAudio low latency
        minSdk = 29
        targetSdk = 35
        versionCode = 1
        versionName = "0.1"
        ndk { abiFilters += "arm64-v8a" }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            // installable as is (sideloading); sign with your own key for anything else
            signingConfig = signingConfigs.getByName("debug")
            // lets android/report.sh dump a frozen game's threads (debuggerd -b); the game's code
            // is native and runs the same, only the Java launcher is affected
            isDebuggable = true
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    // libmuguet.so, built by ../make-android.sh
    sourceSets["main"].jniLibs.srcDir("../../target/android/jniLibs")

    packaging {
        // stored uncompressed and page-aligned: mapped straight from the APK, no copy at install
        jniLibs.useLegacyPackaging = false
    }
}
