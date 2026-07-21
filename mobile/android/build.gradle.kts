// Gradle project that packages KaadanEngine's Rust .so files into an APK.
// `scripts/build_android.sh` builds the cdylib with cargo-ndk and drops the
// per-ABI .so under `mobile/android/jniLibs/<abi>/`, which `jniLibs.srcDirs`
// below picks up. Add a signingConfig for release/store builds.
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
            // arm64-v8a covers essentially all real devices; add "armeabi-v7a"
            // and "x86_64" (emulator) once the arm64 bring-up is verified.
            abiFilters += listOf("arm64-v8a")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            // signingConfig = signingConfigs.getByName("release")
        }
    }

    sourceSets["main"].jniLibs.srcDirs("jniLibs")
    sourceSets["main"].manifest.srcFile("AndroidManifest.xml")
}
