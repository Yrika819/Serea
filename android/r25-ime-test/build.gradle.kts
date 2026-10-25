plugins {
    id("com.android.application")
}

android {
    namespace = "app.serea.android.r25testime"
    compileSdk = 35

    defaultConfig {
        applicationId = "app.serea.android.r25testime"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "1.0-r25-diagnostic"
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}
