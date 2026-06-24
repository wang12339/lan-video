import java.util.Properties
import java.io.FileInputStream

plugins {
    alias(libs.plugins.android.application)
    // Kotlin serialization for compile-time @Serializable KSerializer generation
    alias(libs.plugins.kotlin.serialization)
    id("org.jetbrains.kotlin.plugin.compose")
    id("com.google.devtools.ksp")
}

// Load keystore properties (release signing) — safe to fail for debug builds
val keystorePropsFile = rootProject.file("app/keystore.properties")
val keystoreProps = if (keystorePropsFile.exists()) {
    Properties().apply { load(FileInputStream(keystorePropsFile)) }
} else null

android {
    namespace = "com.lanvideo.player"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.lanvideo.player"
        minSdk = 26
        targetSdk = 35
        versionCode = 3
        versionName = "1.2.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        // Build config fields — override via gradle.properties or CI env
        buildConfigField("String", "SENTRY_DSN", "\"${project.findProperty("SENTRY_DSN") ?: ""}\"")
        buildConfigField("String", "BUILD_VARIANT", "\"${if (keystoreProps != null) "release" else "debug"}\"")
    }

    signingConfigs {
        create("release") {
            if (keystoreProps != null) {
                storeFile = rootProject.file("app/${keystoreProps.getProperty("storeFile")}")
                storePassword = keystoreProps.getProperty("storePassword")
                keyAlias = keystoreProps.getProperty("keyAlias")
                keyPassword = keystoreProps.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        debug {
            isMinifyEnabled = false
            applicationIdSuffix = ".debug"
            versionNameSuffix = "-debug"
        }
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
            signingConfig = if (keystoreProps != null) {
                signingConfigs.getByName("release")
            } else {
                println("⚠️  WARNING: No keystore.properties found — release build will be UNSIGNED!")
                signingConfigs.getByName("debug")
            }
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }

    buildFeatures {
        buildConfig = true
        compose = true
    }

    testOptions {
        unitTests.isReturnDefaultValues = true
    }
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.appcompat)
    implementation(libs.material)
    implementation(libs.androidx.lifecycle.livedata.ktx)
    implementation(libs.androidx.lifecycle.viewmodel.ktx)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.retrofit)
    implementation(libs.okhttp.logging)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.retrofit.kotlinx.serialization.converter)
    implementation(libs.coil)
    implementation(libs.androidx.media3.exoplayer)
    implementation(libs.androidx.media3.ui)
    implementation(libs.androidx.documentfile)
    implementation(libs.sentry.android)
    implementation(libs.androidx.security.crypto)
    
    // Compose BOM
    val composeBom = platform("androidx.compose:compose-bom:2025.04.01")
    implementation(composeBom)

    // Material 3
    implementation("androidx.compose.material3:material3")

    // Compose UI
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    debugImplementation("androidx.compose.ui:ui-tooling")

    // Navigation
    implementation("androidx.navigation:navigation-compose:2.8.9")

    // Activity Compose
    implementation("androidx.activity:activity-compose:1.10.1")

    // Lifecycle
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")

    // Coil (图片加载)
    implementation("io.coil-kt:coil-compose:2.7.0")

    // Koin DI
    implementation("io.insert-koin:koin-android:3.5.6")
    implementation("io.insert-koin:koin-androidx-compose:3.5.6")

    // Room (offline caching)
    implementation("androidx.room:room-runtime:2.6.1")
    implementation("androidx.room:room-ktx:2.6.1")
    ksp("androidx.room:room-compiler:2.6.1")

    testImplementation(libs.junit)
    androidTestImplementation(libs.androidx.junit)
    androidTestImplementation(libs.androidx.espresso.core)

    // Compose UI tests
    androidTestImplementation("androidx.compose.ui:ui-test-junit4")
    debugImplementation("androidx.compose.ui:ui-test-manifest")
}
