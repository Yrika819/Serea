plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
    id("com.google.devtools.ksp")
    id("com.github.jk1.dependency-license-report")
}

val requestedVersionCode = providers.gradleProperty("sereaVersionCode").orNull
val appVersionCode = requestedVersionCode?.let {
    it.toIntOrNull() ?: error("sereaVersionCode must be a positive integer")
} ?: 1
require(appVersionCode > 0) { "sereaVersionCode must be a positive integer" }

// The update-compatibility test creates this short-lived key in RUNNER_TEMP.
// Ordinary PR builds and internal Beta builds use separate signing inputs.
val updateTestStoreFile = providers.environmentVariable("SEREA_UPDATE_TEST_KEYSTORE_PATH").orNull
val updateTestStorePassword = providers.environmentVariable("SEREA_UPDATE_TEST_STORE_PASSWORD").orNull
val updateTestKeyPassword = providers.environmentVariable("SEREA_UPDATE_TEST_KEY_PASSWORD").orNull
val updateTestKeyAlias = providers.environmentVariable("SEREA_UPDATE_TEST_KEY_ALIAS").orNull
val updateTestSigningReady = listOf(
    updateTestStoreFile, updateTestStorePassword, updateTestKeyPassword, updateTestKeyAlias,
).all { !it.isNullOrBlank() }

val betaStoreFile = providers.environmentVariable("SEREA_ANDROID_KEYSTORE_PATH").orNull
val betaStorePassword = providers.environmentVariable("SEREA_ANDROID_KEYSTORE_PASSWORD").orNull
val betaKeyPassword = providers.environmentVariable("SEREA_ANDROID_KEY_PASSWORD").orNull
val betaKeyAlias = providers.environmentVariable("SEREA_ANDROID_KEY_ALIAS").orNull
val betaSigningReady = listOf(betaStoreFile, betaStorePassword, betaKeyPassword, betaKeyAlias)
    .all { !it.isNullOrBlank() }

android {
    namespace = "app.serea.android"
    compileSdk = 35

    defaultConfig {
        applicationId = "app.serea.android"
        minSdk = 26
        targetSdk = 35
        versionCode = appVersionCode
        versionName = "0.1.0-demo"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    if (updateTestSigningReady) {
        signingConfigs.create("ephemeralUpdateTest") {
            storeFile = file(checkNotNull(updateTestStoreFile))
            storePassword = checkNotNull(updateTestStorePassword)
            keyAlias = checkNotNull(updateTestKeyAlias)
            keyPassword = checkNotNull(updateTestKeyPassword)
        }
    }
    if (betaSigningReady) {
        signingConfigs.create("internalBeta") {
            storeFile = file(checkNotNull(betaStoreFile))
            storeType = "PKCS12"
            storePassword = checkNotNull(betaStorePassword)
            keyAlias = checkNotNull(betaKeyAlias)
            keyPassword = checkNotNull(betaKeyPassword)
        }
    }

    buildTypes {
        debug {
            applicationIdSuffix = ".demo"
            if (updateTestSigningReady) signingConfig = signingConfigs.getByName("ephemeralUpdateTest")
        }
        release {
            isMinifyEnabled = false
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
        create("internalBeta") {
            initWith(getByName("release"))
            applicationIdSuffix = ".demo"
            signingConfig = if (betaSigningReady) signingConfigs.getByName("internalBeta") else null
            matchingFallbacks += listOf("release")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    lint { abortOnError = true }
    testOptions { unitTests.isIncludeAndroidResources = true }
}

tasks.register("verifyInternalBetaSigning") {
    group = "verification"
    description = "Fail unless a complete owner-managed internal Beta signing identity is configured."
    doLast {
        val required = mapOf(
            "SEREA_ANDROID_KEYSTORE_PATH" to betaStoreFile,
            "SEREA_ANDROID_KEYSTORE_PASSWORD" to betaStorePassword,
            "SEREA_ANDROID_KEY_ALIAS" to betaKeyAlias,
            "SEREA_ANDROID_KEY_PASSWORD" to betaKeyPassword,
            "SEREA_ANDROID_SIGNING_CERT_SHA256" to providers.environmentVariable("SEREA_ANDROID_SIGNING_CERT_SHA256").orNull,
        )
        val missing = required.filterValues { it.isNullOrBlank() }.keys
        check(missing.isEmpty()) { "Internal Beta signing is unavailable; missing required configuration: ${missing.joinToString()}" }
        check(betaSigningReady) { "Internal Beta signing identity was not loaded by Gradle." }
        check(File(checkNotNull(betaStoreFile)).isFile) { "Internal Beta keystore file is unavailable." }
        val fingerprint = required.getValue("SEREA_ANDROID_SIGNING_CERT_SHA256")!!
        check(fingerprint.replace(":", "").matches(Regex("(?i)[0-9a-f]{64}"))) {
            "SEREA_ANDROID_SIGNING_CERT_SHA256 must contain a SHA-256 certificate fingerprint."
        }
        check(appVersionCode > 1) { "Internal Beta versionCode must be greater than the initial development version." }
    }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2025.03.00")
    implementation(composeBom)
    androidTestImplementation(composeBom)

    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation("androidx.lifecycle:lifecycle-viewmodel-ktx:2.8.7")
    implementation("androidx.room:room-runtime:2.7.0")
    implementation("androidx.room:room-ktx:2.7.0")
    ksp("androidx.room:room-compiler:2.7.0")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("com.google.code.gson:gson:2.12.1")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.1")
    debugImplementation("androidx.compose.ui:ui-tooling")

    testImplementation("junit:junit:4.13.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.10.1")
    testImplementation("androidx.arch.core:core-testing:2.2.0")
    testImplementation("com.squareup.okhttp3:mockwebserver:4.12.0")
    testImplementation("com.squareup.okhttp3:okhttp-tls:4.12.0")
    androidTestImplementation("androidx.test.ext:junit:1.2.1")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.6.1")
    androidTestImplementation("androidx.compose.ui:ui-test-junit4")
    androidTestImplementation("com.squareup.okhttp3:mockwebserver:4.12.0")
    androidTestImplementation("com.squareup.okhttp3:okhttp-tls:4.12.0")
    debugImplementation("androidx.compose.ui:ui-test-manifest")
}

ksp { arg("room.schemaLocation", "$projectDir/schemas") }

dependencyLocking { lockAllConfigurations() }

// AGP 8.9's Android Test Platform configurations resolve older protobuf/Netty
// versions. Keep their test-runner transport dependencies on patched compatible
// lines; this does not add either library to the app runtime unless requested by
// a runtime dependency.
configurations.configureEach {
    resolutionStrategy.eachDependency {
        if (requested.group == "com.google.protobuf" && requested.name in setOf(
                "protobuf-java", "protobuf-javalite", "protobuf-java-util", "protobuf-kotlin",
            )) {
            useVersion("3.25.9")
            because("CVE-2024-7254 affects older protobuf releases")
        }
        if (requested.group == "io.netty") {
            useVersion("4.1.139.Final")
            because("patched 4.1 series for Android Test Platform transport dependencies")
        }
    }
}

licenseReport {
    configurations = arrayOf(
        "debugRuntimeClasspath",
        "releaseRuntimeClasspath",
        "debugUnitTestRuntimeClasspath",
        "debugAndroidTestRuntimeClasspath",
    )
}
