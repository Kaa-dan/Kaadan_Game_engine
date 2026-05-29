// Minimal Gradle scaffold for packaging KaadanEngine's Rust .so files into an APK.
// Wire `jniLibs.srcDirs` to the cargo-ndk output (see scripts/build_android.sh)
// and add a signingConfig for release builds.
plugins {
    id("com.android.application")
}

android {
    namespace = "dev.kaadan.engine"
    compileSdk = 34

    defaultConfig {
        applicationId = "dev.kaadan.engine"
        minSdk = 24
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"
        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            // signingConfig = signingConfigs.getByName("release")
        }
    }

    // sourceSets["main"].jniLibs.srcDirs("../../target/jniLibs")
    sourceSets["main"].manifest.srcFile("AndroidManifest.xml")
}
