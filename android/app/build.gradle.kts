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
val privateVersionCodeText = providers.gradleProperty("sereaPrivateBetaVersionCode").orNull
val privateBetaVersionCode = privateVersionCodeText?.toIntOrNull() ?: if (privateVersionCodeText == null) 2 else
    error("-PsereaPrivateBetaVersionCode must be an integer >= 2.")
if (privateBetaVersionCode < 2) error("-PsereaPrivateBetaVersionCode must be >= 2.")
val privateStorePath = providers.environmentVariable("SEREA_ANDROID_PRIVATE_BETA_KEYSTORE_PATH").orNull
val privateStorePassword = providers.environmentVariable("SEREA_ANDROID_PRIVATE_BETA_KEYSTORE_PASSWORD").orNull
val privateKeyAlias = providers.environmentVariable("SEREA_ANDROID_PRIVATE_BETA_KEY_ALIAS").orNull
val privateKeyPassword = providers.environmentVariable("SEREA_ANDROID_PRIVATE_BETA_KEY_PASSWORD").orNull
val privateExpectedFingerprint = providers.environmentVariable("SEREA_ANDROID_PRIVATE_BETA_CERT_SHA256").orNull
val privateSigningInputsReady = listOf(privateStorePath, privateStorePassword, privateKeyAlias, privateKeyPassword,
    privateExpectedFingerprint).all { !it.isNullOrBlank() }
val betaSigningInputsReady = listOf(
    betaStorePath, betaStorePassword, betaKeyAlias, betaKeyPassword, betaExpectedFingerprint,
).all { !it.isNullOrBlank() }
if (privateSigningInputsReady && betaSigningInputsReady &&
    privateExpectedFingerprint!!.filter { it.isLetterOrDigit() }.equals(
        betaExpectedFingerprint!!.filter { it.isLetterOrDigit() }, ignoreCase = true)) {
    error("Public and Private Beta must use different signing certificates.")
}
val betaValidationHarness = providers.gradleProperty("sereaPersonalBetaValidationHarness").orNull == "true"
// Opt-in is local to the Owner's private Beta invocation. Missing input keeps OAuth compiled off.
val ownerPrivateBetaGoogleEnabled = providers.environmentVariable("SEREA_OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED").orNull == "true"
val ciBuild = providers.environmentVariable("CI").orNull.equals("true", ignoreCase = true) ||
    providers.environmentVariable("GITHUB_ACTIONS").orNull.equals("true", ignoreCase = true)
val privateBetaRequested = gradle.startParameter.taskNames.any { it.contains("PrivateBeta", ignoreCase = true) }
// CI may compile and run this exact no-secret JVM unit-test variant. Every other
// CI invocation leaves the Private Beta variant disabled, and packaging tasks are rejected.
val privateBetaUnitTestFilterArgs = setOf(
    "--tests",
    "app.serea.android.googleauth.PrivateBuildOAuthGateTest",
)
val requestedTaskNames = gradle.startParameter.taskNames.filterNot { it in privateBetaUnitTestFilterArgs }
val privateBetaUnitTestOnlyInvocation = gradle.startParameter.taskNames.isNotEmpty() &&
    requestedTaskNames.isNotEmpty() &&
    requestedTaskNames.all { it.substringAfterLast(':') == "testPrivateBetaUnitTest" }
if (privateBetaRequested && ciBuild && !privateBetaUnitTestOnlyInvocation) {
    error("CI may only run the isolated Private Beta unit-test task; packaging is local-only.")
}
val isolatedNativeRuntimeOptIn = providers.gradleProperty("sereaIsolatedNativeRuntime").orNull == "true"
val nativeIsolationVerifierOnly = gradle.startParameter.taskNames.isNotEmpty() &&
    gradle.startParameter.taskNames.all { it.substringAfterLast(':') == "verifyLocalNativeRuntimeVariantIsolation" }
val nativeRuntimeAbi = providers.gradleProperty("sereaNativeRuntimeAbi").orNull ?: "x86_64"
val llamaCppSource = if (isolatedNativeRuntimeOptIn && !nativeIsolationVerifierOnly) {
    providers.gradleProperty("sereaLlamaCppSource").orNull
        ?: providers.environmentVariable("SEREA_LLAMA_CPP_SOURCE").orNull
        ?: error("Native diagnostic requires -PsereaLlamaCppSource=<pinned clean checkout>.")
} else null
if (isolatedNativeRuntimeOptIn && nativeRuntimeAbi !in setOf("x86_64", "arm64-v8a")) {
    error("sereaNativeRuntimeAbi must be x86_64 or arm64-v8a.")
}
val nativeEnabledBuildTypes = mutableSetOf<String>()
val nativeDisabledBuildTypes = mutableSetOf<String>()
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

    buildFeatures { buildConfig = true }
    if (isolatedNativeRuntimeOptIn && !nativeIsolationVerifierOnly) ndkVersion = "28.2.13676358"

    defaultConfig {
        applicationId = "app.serea.android"
        minSdk = 26
        targetSdk = 35
        versionCode = updateTestVersionCode ?: 1
        versionName = "0.1.0-demo"
        // The Native diagnostic runs only the no-model JNI contract on a disposable emulator.
        // Keep the campaign-gated Formal runner for every ordinary app/test build.
        testInstrumentationRunner = if (isolatedNativeRuntimeOptIn) {
            "androidx.test.runner.AndroidJUnitRunner"
        } else {
            "app.serea.android.SereaIsolatedAndroidJUnitRunner"
        }
        buildConfigField("boolean", "LOCAL_NATIVE_RUNTIME_ENABLED", "false")
        buildConfigField("boolean", "OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED", "false")
        buildConfigField("boolean", "PRIVATE_GOOGLE_AUTH_UI_ENABLED", "false")
        if (isolatedNativeRuntimeOptIn && !nativeIsolationVerifierOnly) {
            ndk { abiFilters += nativeRuntimeAbi }
            externalNativeBuild {
                cmake {
                    arguments += "-DLLAMA_SRC=$llamaCppSource"
                    // JNI progress and failure injection are compiled only when the explicit
                    // isolated Native diagnostic opt-in is active. Product variants stay off.
                    arguments += "-DSEREA_NATIVE_JNI_TEST_SEAMS=ON"
                }
            }
        }
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
        if (privateSigningInputsReady) {
            create("privateBetaOwner") {
                storeFile = file(checkNotNull(privateStorePath))
                storeType = "PKCS12"
                storePassword = checkNotNull(privateStorePassword)
                keyAlias = checkNotNull(privateKeyAlias)
                keyPassword = checkNotNull(privateKeyPassword)
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
            buildConfigField(
                "boolean",
                "LOCAL_NATIVE_RUNTIME_ENABLED",
                isolatedNativeRuntimeOptIn.toString(),
            )
            buildConfigField("String", "LOCAL_NATIVE_RUNTIME_EXPECTED_ABI", "\"$nativeRuntimeAbi\"")
        }
        release {
            isMinifyEnabled = false
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
        create("beta") {
            initWith(getByName("release"))
            applicationIdSuffix = ".beta"
            matchingFallbacks += listOf("release")
            buildConfigField("boolean", "OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED", "false")
            buildConfigField("boolean", "PRIVATE_GOOGLE_AUTH_UI_ENABLED", "false")
            if (betaSigningInputsReady) signingConfig = signingConfigs.getByName("personalBeta")
        }
        create("privateBeta") {
            initWith(getByName("beta"))
            // initWith copies beta's signing config; clear it before applying the
            // independent Private-only configuration. There is no Public fallback.
            signingConfig = null
            applicationIdSuffix = ".privatebeta"
            matchingFallbacks += listOf("beta", "release")
            buildConfigField(
                "boolean",
                "OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED",
                ownerPrivateBetaGoogleEnabled.toString(),
            )
            buildConfigField("boolean", "PRIVATE_GOOGLE_AUTH_UI_ENABLED", "true")
            if (privateSigningInputsReady) signingConfig = signingConfigs.getByName("privateBetaOwner")
        }
    }
    if (isolatedNativeRuntimeOptIn && !nativeIsolationVerifierOnly) {
        externalNativeBuild {
            cmake { path = file("src/main/cpp/CMakeLists.txt"); version = "3.31.6" }
        }
    }

    sourceSets {
        // Instrumentation and the process-death harness use a separate Android
        // application ID and therefore a separate app-private DB/Preferences area.
        getByName("isolated") {
            java.srcDir("src/debug/java")
            manifest.srcFile("src/isolated/AndroidManifest.xml")
        }
        if (isolatedNativeRuntimeOptIn) {
            // JNI ownership smoke tests are compiled only for the explicit native
            // diagnostic build; regular Formal instrumentation identities stay fixed.
            getByName("androidTest") {
                java.srcDir("src/nativeRuntimeDiagnosticAndroidTest/java")
            }
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

androidComponents {
    beforeVariants(selector().all()) { variant ->
        if (ciBuild && variant.buildType == "privateBeta" && !privateBetaUnitTestOnlyInvocation) {
            variant.enable = false
        }
        if (isolatedNativeRuntimeOptIn) {
            if (variant.buildType == "isolated") nativeEnabledBuildTypes += variant.buildType!!
            else {
                nativeDisabledBuildTypes += variant.buildType.orEmpty()
                variant.enable = false
            }
        }
    }
}

tasks.register("verifyLocalNativeRuntimeVariantIsolation") {
    group = "verification"
    doLast {
        check(isolatedNativeRuntimeOptIn) { "Use -PsereaIsolatedNativeRuntime=true." }
        check(nativeEnabledBuildTypes == setOf("isolated")) { "Native enabled variants: $nativeEnabledBuildTypes" }
        check(nativeDisabledBuildTypes.containsAll(setOf("debug", "beta", "privateBeta", "release"))) {
            "Native disabled variants: $nativeDisabledBuildTypes"
        }
    }
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

fun registerBetaSigningGuard(taskName: String, privateVariant: Boolean) = tasks.register(taskName) {
    group = "verification"
    description = if (privateVariant) "Validate Private Beta signing identity." else "Validate Public Beta signing identity."
    outputs.upToDateWhen { false }
    doLast {
        fun failClosed(message: String): Nothing {
            val variant = if (privateVariant) "privateBeta" else "beta"
            layout.buildDirectory.dir("outputs/apk/$variant").get().asFile.deleteRecursively()
            layout.buildDirectory.dir("outputs/bundle/$variant").get().asFile.deleteRecursively()
            error(message)
        }
        fun normalizeFingerprint(value: String, label: String): String {
            val normalized = value.replace(":", "").replace(Regex("\\s"), "").uppercase(Locale.ROOT)
            if (!normalized.matches(Regex("[0-9A-F]{64}"))) failClosed("$label must be a SHA-256 certificate fingerprint.")
            return normalized
        }
        val storePath = if (privateVariant) privateStorePath else betaStorePath
        val storePassword = if (privateVariant) privateStorePassword else betaStorePassword
        val keyAlias = if (privateVariant) privateKeyAlias else betaKeyAlias
        val keyPassword = if (privateVariant) privateKeyPassword else betaKeyPassword
        val expectedFingerprint = if (privateVariant) privateExpectedFingerprint else betaExpectedFingerprint
        val required = mapOf(
            "keystore path" to storePath,
            "keystore password" to storePassword,
            "key alias" to keyAlias,
            "key password" to keyPassword,
            "certificate SHA-256" to expectedFingerprint,
        ).toMutableMap()
        if (privateVariant) {
            // This is an Owner-trusted fingerprint, not a Public key or password.
            required["trusted Public Beta certificate SHA-256"] = betaExpectedFingerprint
            required["Private Beta versionCode"] = privateBetaVersionCode.toString()
        } else {
            required["Public Beta versionCode"] = betaVersionCode?.toString()
        }
        val missing = required.filterValues { it.isNullOrBlank() }.keys
        if (missing.isNotEmpty()) failClosed("Beta packaging blocked; missing ${missing.joinToString()}.")
        if (!privateVariant && checkNotNull(betaVersionCode) <= 1) {
            failClosed("Public Beta versionCode must be greater than 1.")
        }
        val expected = normalizeFingerprint(checkNotNull(expectedFingerprint), "Configured certificate SHA-256")
        if (privateVariant) {
            val trustedPublic = normalizeFingerprint(checkNotNull(betaExpectedFingerprint), "Trusted Public certificate SHA-256")
            if (expected == trustedPublic) failClosed("Public and Private Beta signing certificates must differ.")
        }
        val keystoreFile = file(checkNotNull(storePath))
        if (!keystoreFile.isFile) failClosed("Beta keystore path is not a file.")
        val store = try {
            KeyStore.getInstance("PKCS12").apply {
                keystoreFile.inputStream().use { load(it, checkNotNull(storePassword).toCharArray()) }
            }
        } catch (_: Exception) { failClosed("Beta PKCS12 keystore could not be opened with the configured password.") }
        if (!store.isKeyEntry(checkNotNull(keyAlias))) failClosed("Configured Beta alias is not a private-key entry.")
        val certificate = store.getCertificate(checkNotNull(keyAlias))
            ?: failClosed("Configured Beta alias has no signing certificate.")
        val actual = MessageDigest.getInstance("SHA-256").digest(certificate.encoded)
            .joinToString("") { "%02X".format(Locale.ROOT, it.toInt() and 0xff) }
        if (actual != expected) failClosed("Beta keystore certificate does not match configured SHA-256.")
        val signingKey = try {
            store.getKey(checkNotNull(keyAlias), checkNotNull(keyPassword).toCharArray())
        } catch (_: Exception) { failClosed("Configured Beta key password could not unlock the private key.") }
        if (signingKey !is java.security.PrivateKey) failClosed("Configured Beta alias is not an accessible private signing key.")
    }
}

val validatePublicBetaSigning = registerBetaSigningGuard("validatePublicBetaSigning", privateVariant = false)
val validatePrivateBetaSigning = registerBetaSigningGuard("validatePrivateBetaSigning", privateVariant = true)

// Attach to concrete artifact-producing tasks, not similarly prefixed resource
// tasks such as packagePrivateBetaResources. Aggregate assemble/bundle and task
// abbreviations resolve through these canonical variant nodes.
val publicBetaPackagingTasks = setOf("assembleBeta", "packageBeta", "bundleBeta", "signBetaBundle")
val privateBetaPackagingTasks = setOf(
    "assemblePrivateBeta", "packagePrivateBeta", "bundlePrivateBeta", "signPrivateBetaBundle",
)
tasks.configureEach {
    if (name in privateBetaPackagingTasks) {
        dependsOn(validatePrivateBetaSigning)
    } else if (name in publicBetaPackagingTasks) {
        dependsOn(validatePublicBetaSigning)
    }
}

androidComponents.onVariants { variant ->
    if (variant.buildType == "beta") {
        check(variant.applicationId.get() == "app.serea.android.beta") {
            "Personal Beta applicationId drifted: expected app.serea.android.beta, got ${variant.applicationId.get()}"
        }
        betaVersionCode?.let { code -> variant.outputs.forEach { it.versionCode.set(code) } }
    }
    if (variant.buildType == "privateBeta") {
        check(variant.applicationId.get() == "app.serea.android.privatebeta") {
            "Private Beta applicationId drifted: expected app.serea.android.privatebeta, got ${variant.applicationId.get()}"
        }
        variant.outputs.forEach { it.versionCode.set(privateBetaVersionCode) }
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
