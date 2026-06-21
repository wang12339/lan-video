plugins {
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.kotlin.serialization) apply false
    kotlin("plugin.compose") version "2.0.21" apply false
}
