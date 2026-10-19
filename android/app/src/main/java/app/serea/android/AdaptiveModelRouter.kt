package app.serea.android

/** Values are UNKNOWN unless supported by a source captured when the catalog entry was verified. */
sealed interface Knowledge<out T> {
    data object UNKNOWN : Knowledge<Nothing>
    data class Known<T>(val value: T) : Knowledge<T>
}
enum class ModelSelectionMode { AUTO, MANUAL }
enum class AutoRoutingPolicy { BALANCED, COST_PRIORITY, QUALITY_PRIORITY }
enum class ModelAvailability { AVAILABLE, UNAVAILABLE, UNKNOWN }
enum class ModelTask { SIMPLE, STANDARD, COMPLEX, STRUCTURED, LONG_CONTEXT, TOOL_REQUIRED }
enum class ModelCapability { TEXT_INPUT, IMAGE_INPUT, TOOL_CALLING, STRUCTURED_OUTPUT, REASONING }

private fun validModelId(value: String) = value.matches(Regex("[A-Za-z0-9][A-Za-z0-9._:/-]{0,127}")) && !value.contains("://")
private fun Knowledge<Double>.knownNumber(): Double? = (this as? Knowledge.Known<*>)?.value as? Double
internal fun Knowledge<Int>.knownInt(): Int? = (this as? Knowledge.Known<*>)?.value as? Int
private fun Knowledge<String>.knownString(): String? = (this as? Knowledge.Known<*>)?.value as? String

data class ModelPricing(
    val inputPerMillion: Knowledge<Double>,
    val outputPerMillion: Knowledge<Double>,
    val currencyCode: Knowledge<String> = Knowledge.UNKNOWN,
) {
    init {
        listOfNotNull(inputPerMillion.knownNumber(), outputPerMillion.knownNumber()).forEach { require(it.isFinite() && it >= 0) }
        currencyCode.knownString()?.let { require(it.matches(Regex("[A-Z]{3}"))) }
    }
}
data class ModelCapabilities(
    val input: Set<String> = emptySet(),
    val toolCalling: Knowledge<Boolean> = Knowledge.UNKNOWN,
    val structuredOutput: Knowledge<Boolean> = Knowledge.UNKNOWN,
    val reasoning: Knowledge<Boolean> = Knowledge.UNKNOWN,
)
data class ModelDescriptor(
    val id: String,
    val providerId: String,
    val displayName: String,
    val contextWindow: Knowledge<Int> = Knowledge.UNKNOWN,
    val maxOutput: Knowledge<Int> = Knowledge.UNKNOWN,
    val capabilities: ModelCapabilities = ModelCapabilities(),
    val pricing: ModelPricing = ModelPricing(Knowledge.UNKNOWN, Knowledge.UNKNOWN),
    val availability: ModelAvailability = ModelAvailability.UNKNOWN,
    val authorized: Boolean = false,
    val lastVerifiedAt: String? = null,
    val source: String? = null,
) {
    init {
        require(validModelId(id)); require(providerId.matches(Regex("[a-z0-9-]{1,64}")))
        require(displayName.isNotBlank())
        contextWindow.knownInt()?.let { require(it > 0) }
        maxOutput.knownInt()?.let { require(it > 0) }
    }
}

interface ModelCatalog { fun all(): List<ModelDescriptor> }
class LocalModelCatalog(entries: List<ModelDescriptor> = emptyList()) : ModelCatalog {
    @Volatile private var entries = entries.toList()
    init { validate(entries) }
    override fun all(): List<ModelDescriptor> = entries.toList()
    fun replace(entries: List<ModelDescriptor>) { validate(entries); this.entries = entries.toList() }
    private fun validate(entries: List<ModelDescriptor>) { require(entries.map { it.id }.distinct().size == entries.size) }
}

data class ModelRoutingPolicy(
    val mode: ModelSelectionMode = ModelSelectionMode.AUTO,
    val autoPolicy: AutoRoutingPolicy = AutoRoutingPolicy.BALANCED,
    val manualModelId: String? = null,
    val maxEstimatedCost: Double? = null,
    val budgetCurrencyCode: String? = null,
    val maxInputTokens: Int? = null,
    val requestedOutputTokens: Int = 512,
) {
    init {
        require(maxEstimatedCost == null || (maxEstimatedCost.isFinite() && maxEstimatedCost >= 0))
        require(maxEstimatedCost == null || budgetCurrencyCode?.matches(Regex("[A-Z]{3}")) == true)
        require(maxInputTokens == null || maxInputTokens > 0); require(requestedOutputTokens > 0)
    }
}
data class RoutingRequest(
    val text: String,
    val contextCharacters: Int = 0,
    val requestedOutputTokens: Int = 512,
    val requiresStructuredOutput: Boolean = false,
    val requiresTools: Boolean = false,
    val requiredInputFormat: String = "text/plain",
) {
    init { require(contextCharacters >= 0 && requestedOutputTokens > 0) }
}
data class CandidateExclusion(val modelId: String, val reason: String)
data class EstimatedModelCost(val amount: Double, val currencyCode: String, val estimatedInputTokens: Long, val reservedOutputTokens: Int)
data class ModelSelectionResult(
    val model: ModelDescriptor?, val mode: ModelSelectionMode, val task: ModelTask,
    val reason: String, val constraints: List<String>, val excluded: List<CandidateExclusion>,
    val unknownFields: List<String>, val estimatedCost: EstimatedModelCost?,
) {
    val selectedModelId: String? get() = model?.id
}

/** Deterministic, device-local rules. This component never performs egress or fallback. */
class AdaptiveModelRouter(private val catalog: ModelCatalog) {
    fun classify(request: RoutingRequest): ModelTask = when {
        request.requiresTools -> ModelTask.TOOL_REQUIRED
        request.requiresStructuredOutput -> ModelTask.STRUCTURED
        request.contextCharacters > LONG_CONTEXT_CHARS -> ModelTask.LONG_CONTEXT
        request.text.length > COMPLEX_CHARS || listOf("compare", "analyze", "step by step", "trade-off", "比較", "分析").any { request.text.contains(it, true) } -> ModelTask.COMPLEX
        request.text.length <= SIMPLE_CHARS -> ModelTask.SIMPLE
        else -> ModelTask.STANDARD
    }

    fun select(request: RoutingRequest, policy: ModelRoutingPolicy): ModelSelectionResult {
        val task = classify(request)
        val constraints = mutableListOf("端末内の決定的ルール", "利用可能かつ権限確認済み", "必要能力とContext/出力上限", "入力Token数は文字数×4の保守推定")
        if (policy.autoPolicy == AutoRoutingPolicy.QUALITY_PRIORITY) constraints += "実測品質データなし: 品質スコアを作らずID順で決定"
        val excluded = mutableListOf<CandidateExclusion>()
        val choices = catalog.all().sortedBy { it.id }
        val selected = if (policy.mode == ModelSelectionMode.MANUAL) {
            val id = policy.manualModelId
            val item = choices.firstOrNull { it.id == id }
            when {
                id == null -> excluded.add(CandidateExclusion("", "Manualモデルが未指定")); null
                item == null -> excluded.add(CandidateExclusion(id, "Catalog未登録")); null
                else -> { val reason = rejection(item, request, policy, task); if (reason != null) excluded.add(CandidateExclusion(id, reason)); else item }
            }
        } else {
            val viable = choices.mapNotNull { item -> rejection(item, request, policy, task)?.let { excluded.add(CandidateExclusion(item.id, it)); null } ?: item }
            val currencies = viable.mapNotNull { it.pricing.currencyCode.knownString() }.distinct()
            if (policy.autoPolicy == AutoRoutingPolicy.QUALITY_PRIORITY || currencies.size != 1 ||
                viable.any { it.pricing.currencyCode.knownString() == null }) {
                if (policy.autoPolicy != AutoRoutingPolicy.QUALITY_PRIORITY) constraints += "価格通貨を比較できないためID順"
                viable.minByOrNull { it.id }
            } else viable.minWithOrNull(compareBy<ModelDescriptor>({ scoreCost(it, request) }, { it.id }))
        }
        val unknowns = selected?.let(::unknowns).orEmpty()
        val estimatedCost = selected?.let { estimateCost(it, request) }
        val reason = if (selected == null) {
            if (policy.mode == ModelSelectionMode.MANUAL) "指定モデルは利用できません。別モデルへ自動切替はしません。"
            else "条件を満たす登録済みモデルがありません。利用可能なモデルを確認してください。"
        } else "${taskLabel(task)}として分類し、${selected.id}を明示された能力・利用条件に基づいて選択しました。${if (unknowns.isNotEmpty()) "未確認項目があります。" else ""}"
        if (policy.maxEstimatedCost != null) constraints += "推定費用上限 ${policy.maxEstimatedCost}"
        if (policy.maxInputTokens != null) constraints += "入力上限 ${policy.maxInputTokens} tokens"
        constraints += "要求出力 ${request.requestedOutputTokens} tokens"
        return ModelSelectionResult(selected, policy.mode, task, reason, constraints, excluded, unknowns, estimatedCost)
    }

    private fun rejection(m: ModelDescriptor, r: RoutingRequest, p: ModelRoutingPolicy, task: ModelTask): String? {
        if (m.source.isNullOrBlank() || m.lastVerifiedAt.isNullOrBlank()) return "情報源または最終確認日時が未確認"
        if (m.availability != ModelAvailability.AVAILABLE) return "利用可否が${if (m.availability == ModelAvailability.UNKNOWN) "未確認" else "利用不可"}"
        if (!m.authorized) return "利用権限が未確認"
        if (r.requiredInputFormat !in m.capabilities.input) return "必要な入力形式が未確認または非対応"
        val inputTokens = Math.multiplyExact(r.contextCharacters.toLong() + r.text.length, 4L)
        val context = m.contextWindow.knownInt() ?: return "Context上限が未確認"
        if (p.maxInputTokens?.let { inputTokens > it } == true || inputTokens + r.requestedOutputTokens > context.toLong()) return "Context上限不足"
        val output = m.maxOutput.knownInt() ?: return "最大出力が未確認"
        if (r.requestedOutputTokens > output) return "最大出力不足"
        if (r.requiresTools && m.capabilities.toolCalling != Knowledge.Known(true)) return "Tool Calling能力が未確認または非対応"
        if (r.requiresStructuredOutput && m.capabilities.structuredOutput != Knowledge.Known(true)) return "Structured Output能力が未確認または非対応"
        if (task == ModelTask.COMPLEX && m.capabilities.reasoning != Knowledge.Known(true)) return "複雑な分析に必要なReasoning能力が未確認または非対応"
        val currency = m.pricing.currencyCode.knownString()
        val inputPrice = m.pricing.inputPerMillion.knownNumber()
        val outputPrice = m.pricing.outputPerMillion.knownNumber()
        val priceKnown = inputPrice != null && outputPrice != null && currency != null
        if (p.autoPolicy == AutoRoutingPolicy.COST_PRIORITY && !priceKnown) return "料金が未確認のため安価と判断できない"
        if (p.maxEstimatedCost != null) {
            if (!priceKnown || currency != p.budgetCurrencyCode) return "料金または通貨が未確認のため予算適合を確認できない"
            val estimate = (r.contextCharacters.toDouble() + r.text.length.toDouble()) * 4.0 * checkNotNull(inputPrice) / 1_000_000 + r.requestedOutputTokens * checkNotNull(outputPrice) / 1_000_000
            if (estimate > p.maxEstimatedCost) return "推定費用上限超過"
        }
        return null
    }
    private fun scoreCost(m: ModelDescriptor, r: RoutingRequest): Double {
        val price = m.pricing.inputPerMillion.knownNumber() ?: Double.POSITIVE_INFINITY
        val output = m.pricing.outputPerMillion.knownNumber() ?: Double.POSITIVE_INFINITY
        return when {
            m.pricing.inputPerMillion.knownNumber() != null && m.pricing.outputPerMillion.knownNumber() != null && m.pricing.currencyCode.knownString() != null -> ((r.contextCharacters.toDouble() + r.text.length.toDouble()) * 4.0 * price + r.requestedOutputTokens * output) / 1_000_000
            else -> Double.POSITIVE_INFINITY
        }
    }
    private fun estimateCost(m: ModelDescriptor, r: RoutingRequest): EstimatedModelCost? {
        val input = m.pricing.inputPerMillion.knownNumber() ?: return null
        val output = m.pricing.outputPerMillion.knownNumber() ?: return null
        val currency = m.pricing.currencyCode.knownString() ?: return null
        val inputTokens = Math.multiplyExact(r.contextCharacters.toLong() + r.text.length, 4L)
        val amount = inputTokens * input / 1_000_000 + r.requestedOutputTokens * output / 1_000_000
        if (!amount.isFinite()) return null
        return EstimatedModelCost(amount, currency, inputTokens, r.requestedOutputTokens)
    }
    private fun unknowns(m: ModelDescriptor) = buildList {
        if (m.contextWindow == Knowledge.UNKNOWN) add("Context Window")
        if (m.maxOutput == Knowledge.UNKNOWN) add("最大出力")
        if (m.capabilities.input.isEmpty()) add("入力形式")
        if (m.availability == ModelAvailability.UNKNOWN) add("利用可否")
        if (m.capabilities.toolCalling == Knowledge.UNKNOWN) add("Tool Calling")
        if (m.capabilities.structuredOutput == Knowledge.UNKNOWN) add("Structured Output")
        if (m.pricing.inputPerMillion == Knowledge.UNKNOWN || m.pricing.outputPerMillion == Knowledge.UNKNOWN || m.pricing.currencyCode == Knowledge.UNKNOWN) add("料金/通貨")
        if (m.capabilities.reasoning == Knowledge.UNKNOWN) add("Reasoning情報")
        if (m.source == null || m.lastVerifiedAt == null) add("情報源/最終確認日時")
    }
    companion object { const val SIMPLE_CHARS = 120; const val COMPLEX_CHARS = 600; const val LONG_CONTEXT_CHARS = 12_000 }
}

/** Resolves locally before delegating. A missing/manual unavailable model is an error, never a fallback. */
internal class RoutedModelChatPort(
    private val delegate: ModelChatPort,
    private val router: AdaptiveModelRouter,
    private val policy: () -> ModelRoutingPolicy,
) : ModelChatPort {
    override suspend fun send(request: ModelChatRequest): ModelChatResponse {
        val current = policy()
        val selected = router.select(RoutingRequest(request.message, request.context.sumOf { it.text.length }), current)
        val model = selected.model ?: throw ModelChatFailure(selected.reason)
        return delegate.send(request.copy(selectedModelId = model.id))
    }
}

private fun taskLabel(task: ModelTask) = when (task) {
    ModelTask.SIMPLE -> "簡単な会話"; ModelTask.STANDARD -> "通常の文章生成"; ModelTask.COMPLEX -> "複雑な分析";
    ModelTask.STRUCTURED -> "構造化出力"; ModelTask.LONG_CONTEXT -> "長いContext"; ModelTask.TOOL_REQUIRED -> "Tool Calling"
}

/** Settings are ordinary preferences, separated from encrypted provider credentials. */
class ModelSelectionSettings(private val preferences: android.content.SharedPreferences) {
    fun load(): ModelRoutingPolicy {
        val mode = runCatching { ModelSelectionMode.valueOf(preferences.getString("mode", null) ?: "AUTO") }.getOrDefault(ModelSelectionMode.AUTO)
        val policy = runCatching { AutoRoutingPolicy.valueOf(preferences.getString("policy", null) ?: "BALANCED") }.getOrDefault(AutoRoutingPolicy.BALANCED)
        val currency = preferences.getString("budgetCurrency", null)?.takeIf { it.matches(Regex("[A-Z]{3}")) }
        val amount = preferences.getString("maxEstimatedCost", null)?.toDoubleOrNull()?.takeIf { it.isFinite() && it >= 0 }
        return ModelRoutingPolicy(mode, policy, preferences.getString("modelId", null)?.takeIf(::validModelId),
            maxEstimatedCost = amount?.takeIf { currency != null }, budgetCurrencyCode = currency?.takeIf { amount != null })
    }
    fun save(value: ModelRoutingPolicy) {
        require(value.manualModelId == null || validModelId(value.manualModelId))
        require(value.maxEstimatedCost == null || value.budgetCurrencyCode?.matches(Regex("[A-Z]{3}")) == true)
        check(preferences.edit().putString("mode", value.mode.name).putString("policy", value.autoPolicy.name)
            .putString("modelId", value.manualModelId).putString("maxEstimatedCost", value.maxEstimatedCost?.toString())
            .putString("budgetCurrency", value.budgetCurrencyCode).commit())
    }
}
