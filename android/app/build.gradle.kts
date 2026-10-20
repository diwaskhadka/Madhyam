plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

// One version for every platform: the repository's VERSION file (see docs/RELEASING.md).
// versionCode must always grow on Android: 0.9.0 → 900, 1.2.3 → 10203.
val appVersion: String = rootProject.file("../VERSION").readText().trim()
val appVersionCode: Int = appVersion.split(".").map { it.toInt() }.let { (a, b, c) -> a * 10000 + b * 100 + c }

// The display name, shared by every platform: branding/brand.ini (see branding/README.md).
val brand: Map<String, String> = rootProject.file("../branding/brand.ini").readLines(Charsets.UTF_8)
    .map { it.trim() }.filter { "=" in it && !it.startsWith(";") }
    .associate { it.substringBefore("=").trim() to it.substringAfter("=").trim() }

android {
    namespace = "dev.madhyam"
    compileSdk = 35

    defaultConfig {
        // Fixed after the first release: Android treats another id as another app. The Kotlin
        // package (namespace) keeps the project's code name.
        applicationId = "dev.madhyam"
        // Android 5.0: the oldest Android TV release still found on cheap boxes.
        minSdk = 21
        targetSdk = 35
        versionCode = appVersionCode
        versionName = appVersion
        resValue("string", "app_name", brand.getValue("name"))
        buildConfigField("String", "APP_NAME", "\"${brand.getValue("name")}\"")
        buildConfigField("String", "APP_NATIVE_NAME", "\"${brand.getValue("native_name")}\"")
        ndk {
            // Must match the ABIs built by cargo-ndk (see .github/workflows/ci.yml).
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }
    }

    // Release key from the environment (the release workflow fills it from repository secrets,
    // see docs/RELEASING.md). Without it the debug key signs, for test builds only: an APK signed
    // with another key cannot update an installed release.
    val keystore = System.getenv("ANDROID_KEYSTORE_PATH")?.takeIf { file(it).exists() }
    signingConfigs {
        if (keystore != null) {
            create("release") {
                storeFile = file(keystore)
                storePassword = System.getenv("ANDROID_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("ANDROID_KEY_ALIAS")
                keyPassword = System.getenv("ANDROID_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.findByName("release") ?: signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures {
        compose = true
        buildConfig = true
    }

    sourceSets["main"].apply {
        // The Kotlin binding lives next to the other bindings; compile it from there
        // so there is exactly one copy.
        java.srcDir("../../bindings/kotlin")
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2025.08.00")
    implementation(composeBom)
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3") // text fields (TV material has none)
    implementation("androidx.compose.material:material-icons-extended") // R8 strips the unused icons
    implementation("androidx.tv:tv-material:1.0.0")
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.9.2")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.9.2")

    implementation("androidx.media3:media3-exoplayer:1.5.1")
    implementation("androidx.media3:media3-exoplayer-hls:1.5.1")
    implementation("androidx.media3:media3-exoplayer-dash:1.5.1")
    implementation("androidx.media3:media3-ui:1.5.1")

    implementation("io.coil-kt:coil-compose:2.7.0")
    implementation("net.java.dev.jna:jna:5.15.0@aar")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("com.google.zxing:core:3.5.3") // QR code of the Buy me a coffee link (made on the TV, offline)
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")
}
