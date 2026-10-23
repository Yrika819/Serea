import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
    id("com.google.devtools.ksp")
    id("com.github.jk1.dependency-license-report")
}

val betaVersionCode = providers.gradleProperty("sereaBetaVersionCode").orNull?.toIntOrNull()
val betaValidationHarness = providers.gradleProperty("sereaPersonalBetaValidationHarness").orNull == "true"
val updateCompatibilityTest = providers.gradleProperty("sereaUpdateCompatibilityTest").orNull == "true"
val updateTestStorePath = providers.environmentVariable("SEREA_UPDATE_TEST_KEYSTORE_PATH").orNull
val updateTestStorePassword = providers.environmentVariable("SEREA_UPDATE_TEST_STORE_PASSWORD").orNull
val updateTestKeyAlias = providers.environmentVariable("SEREA_UPDATE_TEST_KEY_ALIAS").orNull
val updateTestKeyPassword = providers.environmentVariable("SEREA_UPDATE_TEST_KEY_PASSWORD").orNull
val updateTestSigningInputsReady = listOf(
    updateTestStorePath, updateTestStorePassword, updateTestKeyAlias, updateTestKeyPassword,
).all { !it.isNullOrBlank() }

if (updateCompatibilityTest && !updateTestSigningInputsReady) {
    error("The isolated update compatibility variant requires its disposable test keystore inputs.")
}

android {
    namespace = "app.serea.android"
    compileSdk = 35
    testBuildType = "isolated"

    defaultConfig {
        applicationId = "app.serea.android"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0-demo"
        testInstrumentationRunner = "app.serea.android.SereaIsolatedAndroidJUnitRunner"
    }

    signingConfigs {
        if (updateTestSigningInputsReady) {
            create("ephemeralUpdateTest") {
                storeFile = file(checkNotNull(updateTestStorePath))
                storeType = "PKCS12"
                storePassword = checkNotNull(updateTestStorePassword)
                keyAlias = checkNotNull(updateTestKeyAlias)
                keyPassword = checkNotNull(updateTestKeyPassword)
            }
        }
    }

    buildTypes {
        debug {
            applicationIdSuffix = if (updateCompatibilityTest) ".updatecompat" else ".demo"
            if (updateCompatibilityTest) signingConfig = signingConfigs.getByName("ephemeralUpdateTest")
        }
        create("isolated") {
            initWith(getByName("debug"))
            applicationIdSuffix = ".isolated"
            matchingFallbacks += listOf("debug")
        }
        release {
            isMinifyEnabled = false
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
        create("beta") {
            initWith(getByName("release"))
            applicationIdSuffix = ".beta"
            matchingFallbacks += listOf("release")
            // The Owner's long-lived Beta key is intentionally not configured in Gradle.
            // Gradle builds an unsigned APK; the isolated release signer signs it later.
        }
    }

    sourceSets {
        // Instrumentation and the process-death harness use a separate Android
        // application ID and therefore a separate app-private DB/Preferences area.
        getByName("isolated") {
            java.srcDir("src/debug/java")
            manifest.srcFile("src/debug/AndroidManifest.xml")
        }
        if (betaValidationHarness) {
            // Only the disposable-emulator verifier opts into the debug-only
            // process-death receiver. The protected signing workflow never sets this.
            getByName("beta") {
                java.srcDir("src/debug/java")
                manifest.srcFile("src/betaValidation/AndroidManifest.xml")
            }
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    lint { abortOnError = true }
    testOptions { unitTests.isIncludeAndroidResources = true }
}


androidComponents.onVariants { variant ->
    if (variant.buildType == "beta") {
        check(variant.applicationId.get() == "app.serea.android.beta") {
            "Personal Beta applicationId drifted: expected app.serea.android.beta, got ${variant.applicationId.get()}"
        }
        betaVersionCode?.let { code -> variant.outputs.forEach { it.versionCode.set(code) } }
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
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
    implementation("com.google.android.gms:play-services-auth:22.0.0")
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
    // Compose test activities must be declared in the isolated target APK so
    // ActivityScenario launches them in the app process, never in the test APK.
    add("isolatedImplementation", "androidx.compose.ui:ui-test-manifest")
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
        "isolatedRuntimeClasspath",
        "betaRuntimeClasspath",
        "debugUnitTestRuntimeClasspath",
        "isolatedUnitTestRuntimeClasspath",
        "isolatedAndroidTestRuntimeClasspath",
    )
}
