import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import java.security.KeyStore
import java.security.MessageDigest
import java.util.Locale

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
    id("com.google.devtools.ksp")
    id("com.github.jk1.dependency-license-report")
}

val betaStorePath = providers.environmentVariable("SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PATH").orNull
val betaStorePassword = providers.environmentVariable("SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PASSWORD").orNull
val betaKeyAlias = providers.environmentVariable("SEREA_ANDROID_PERSONAL_BETA_KEY_ALIAS").orNull
val betaKeyPassword = providers.environmentVariable("SEREA_ANDROID_PERSONAL_BETA_KEY_PASSWORD").orNull
val betaExpectedFingerprint = providers.environmentVariable("SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256").orNull
val betaVersionCode = providers.gradleProperty("sereaBetaVersionCode").orNull?.toIntOrNull()
val betaSigningInputsReady = listOf(
    betaStorePath, betaStorePassword, betaKeyAlias, betaKeyPassword, betaExpectedFingerprint,
).all { !it.isNullOrBlank() }
val betaValidationHarness = providers.gradleProperty("sereaPersonalBetaValidationHarness").orNull == "true"
val updateCompatibilityTest = providers.gradleProperty("sereaUpdateCompatibilityTest").orNull == "true"
val updateTestVersionCode = providers.gradleProperty("sereaVersionCode").orNull?.toIntOrNull()
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
if (updateCompatibilityTest && (updateTestVersionCode == null || updateTestVersionCode <= 0)) {
    error("The isolated update compatibility variant requires a positive -PsereaVersionCode.")
}

android {
    namespace = "app.serea.android"
    compileSdk = 35
    testBuildType = "isolated"

    defaultConfig {
        applicationId = "app.serea.android"
        minSdk = 26
        targetSdk = 35
        versionCode = updateTestVersionCode ?: 1
        versionName = "0.1.0-demo"
        // Ordinary Chat stays on the deterministic Demo route until a separately reviewed release.
        buildConfigField("boolean", "LOCAL_MODEL_CHAT_ENABLED", "false")
        testInstrumentationRunner = "app.serea.android.SereaIsolatedAndroidJUnitRunner"
    }

    signingConfigs {
        if (betaSigningInputsReady) {
            create("personalBeta") {
                storeFile = file(checkNotNull(betaStorePath))
                storeType = "PKCS12"
                storePassword = checkNotNull(betaStorePassword)
                keyAlias = checkNotNull(betaKeyAlias)
                keyPassword = checkNotNull(betaKeyPassword)
            }
        }
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
            // Instrumentation's process-death harness uses `run-as` to inspect this app's
            // private database, so the disposable isolated target must remain debuggable.
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
            if (betaSigningInputsReady) signingConfig = signingConfigs.getByName("personalBeta")
        }
    }

    sourceSets {
        // Instrumentation and the process-death harness use a separate Android
        // application ID and therefore a separate app-private DB/Preferences area.
        getByName("isolated") {
            java.srcDir("src/debug/java")
            manifest.srcFile("src/isolated/AndroidManifest.xml")
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
    buildFeatures { buildConfig = true }
    lint { abortOnError = true }
    testOptions { unitTests.isIncludeAndroidResources = true }
}

// AGP omits dependency baseline profiles from debuggable APKs. The isolated target stays
// debuggable for its `run-as` based storage/process-death harness, so copy AGP's compiled
// variant profile into its asset set explicitly and fail if either artifact is missing.
val isolatedBaselineProfileAssetDir = layout.buildDirectory.dir("generated/isolatedBaselineProfileAssets")
val copyIsolatedBaselineProfileAssets = tasks.register<Copy>("copyIsolatedBaselineProfileAssets") {
    dependsOn("compileIsolatedArtProfile")
    val profile = layout.buildDirectory.file(
        "intermediates/binary_art_profile/isolated/compileIsolatedArtProfile/baseline.prof",
    )
    val metadata = layout.buildDirectory.file(
        "intermediates/binary_art_profile_metadata/isolated/compileIsolatedArtProfile/baseline.profm",
    )
    from(profile)
    from(metadata)
    into(isolatedBaselineProfileAssetDir.map { it.dir("dexopt") })
    doFirst {
        listOf(profile.get().asFile, metadata.get().asFile).forEach { artifact ->
            check(artifact.isFile && artifact.length() > 0L) {
                "Required isolated baseline profile artifact is missing or empty: ${artifact.path}"
            }
        }
    }
}
android.sourceSets.getByName("isolated").assets.srcDir(isolatedBaselineProfileAssetDir)
tasks.matching { it.name == "mergeIsolatedAssets" }.configureEach {
    dependsOn(copyIsolatedBaselineProfileAssets)
}

val validatePersonalBetaSigning = tasks.register("validatePersonalBetaSigning") {
    group = "verification"
    description = "Require and verify the Owner-managed Personal Beta signing identity before packaging."
    outputs.upToDateWhen { false }
    doLast {
        fun failClosed(message: String): Nothing {
            layout.buildDirectory.dir("outputs/apk/beta").get().asFile.deleteRecursively()
            layout.buildDirectory.dir("outputs/bundle/beta").get().asFile.deleteRecursively()
            error(message)
        }

        val required = mapOf(
            "SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PATH" to betaStorePath,
            "SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PASSWORD" to betaStorePassword,
            "SEREA_ANDROID_PERSONAL_BETA_KEY_ALIAS" to betaKeyAlias,
            "SEREA_ANDROID_PERSONAL_BETA_KEY_PASSWORD" to betaKeyPassword,
            "SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256" to betaExpectedFingerprint,
            "-PsereaBetaVersionCode" to betaVersionCode?.toString(),
        )
        val missing = required.filterValues { it.isNullOrBlank() }.keys
        if (missing.isNotEmpty()) {
            failClosed("Personal Beta APK packaging is blocked; missing required Owner signing configuration: ${missing.joinToString()}")
        }

        val keystoreFile = file(checkNotNull(betaStorePath))
        if (!keystoreFile.isFile) failClosed("Personal Beta keystore path is not a file.")
        if (checkNotNull(betaVersionCode) <= 1) {
            failClosed("Personal Beta versionCode must be greater than 1; provide -PsereaBetaVersionCode.")
        }

        val expected = checkNotNull(betaExpectedFingerprint)
            .replace(":", "")
            .replace(Regex("\\s"), "")
            .uppercase(Locale.ROOT)
        if (!expected.matches(Regex("[0-9A-F]{64}"))) {
            failClosed("SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256 must be a SHA-256 certificate fingerprint.")
        }

        val store = try {
            KeyStore.getInstance("PKCS12").apply {
                keystoreFile.inputStream().use { load(it, checkNotNull(betaStorePassword).toCharArray()) }
            }
        } catch (_: Exception) {
            failClosed("Personal Beta PKCS12 keystore could not be opened with the configured password.")
        }
        if (!store.isKeyEntry(checkNotNull(betaKeyAlias))) {
            failClosed("The configured Personal Beta alias is not a private-key entry in the PKCS12 keystore.")
        }
        val certificate = store.getCertificate(checkNotNull(betaKeyAlias))
            ?: failClosed("The configured Personal Beta alias has no signing certificate.")
        val actual = MessageDigest.getInstance("SHA-256").digest(certificate.encoded)
            .joinToString("") { "%02X".format(Locale.ROOT, it.toInt() and 0xff) }
        if (actual != expected) {
            failClosed("Personal Beta keystore certificate does not match SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256.")
        }
        val privateKey = try {
            store.getKey(checkNotNull(betaKeyAlias), checkNotNull(betaKeyPassword).toCharArray())
        } catch (_: Exception) {
            failClosed("Personal Beta key password could not unlock the configured private key.")
        }
        if (privateKey !is java.security.PrivateKey) {
            failClosed("The configured Personal Beta alias is not an accessible private signing key.")
        }
    }
}

tasks.matching { it.name in setOf("assembleBeta", "packageBeta", "bundleBeta", "signBetaBundle") }
    .configureEach { dependsOn(validatePersonalBetaSigning) }

tasks.configureEach {
    if (name.contains("Beta") && name != "validatePersonalBetaSigning") {
        mustRunAfter(validatePersonalBetaSigning)
    }
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
    androidTestImplementation("androidx.test.uiautomator:uiautomator:2.3.0")
    androidTestImplementation("androidx.profileinstaller:profileinstaller:1.4.0")
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
        if (requested.group == "com.google.guava" && requested.name == "guava") {
            useVersion("33.7.2-jre")
            because("CVE-2026-102554 affects Guava through 33.7.1; app lock consumers use the JVM variant")
        }
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
