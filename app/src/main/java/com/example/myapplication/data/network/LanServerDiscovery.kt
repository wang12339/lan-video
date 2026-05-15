package com.example.myapplication.data.network

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import com.example.myapplication.ConnectionState
import com.example.myapplication.MyApplication
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import java.net.Inet4Address
import java.util.concurrent.TimeUnit

/**
 * 自动探测同网段内监听 8082 的 LAN 视频服务（/server/info 返回 200 且为 JSON）
 *
 * 策略：先快速探测「常见 / 本机同段」的稀疏列表；未命中再对同网段 /24 全量并行扫描（短超时，避免用户手动填地址）。
 */
object LanServerDiscovery {
    private const val LAN_PORT = 8082
    private const val FULL_SCAN_CONCURRENCY = 36

    /** 单点校验（当前已保存地址等）用稍长超时，避免误杀慢网 */
    private val probeClient by lazy {
        OkHttpClient.Builder()
            .connectTimeout(2, TimeUnit.SECONDS)
            .readTimeout(2, TimeUnit.SECONDS)
            .writeTimeout(2, TimeUnit.SECONDS)
            .build()
    }

    /** 批量扫网用短连接超时，全 /24 也能在可接受时间内跑完 */
    private val fastProbeClient by lazy {
        OkHttpClient.Builder()
            .connectTimeout(900, TimeUnit.MILLISECONDS)
            .readTimeout(2, TimeUnit.SECONDS)
            .writeTimeout(2, TimeUnit.SECONDS)
            .build()
    }

    @Volatile
    private var lastProbeTimeMs: Long = 0L
    private const val DEBOUNCE_MS = 1_500L

    private fun normalizeBaseUrl(url: String): String {
        val t = url.trim()
        return if (t.endsWith("/")) t else "$t/"
    }

    private suspend fun isLanVideoServiceReachable(
        baseUrl: String,
        client: OkHttpClient = probeClient
    ): Boolean = withContext(Dispatchers.IO) {
        val normalized = normalizeBaseUrl(baseUrl)
        // /server/info is a public endpoint (no auth needed), returns {"version":"1.0"}
        val url = "${normalized}server/info"
        return@withContext try {
            val response = client.newCall(
                Request.Builder().url(url).get().build()
            ).execute()
            response.use { r ->
                if (!r.isSuccessful) return@use false
                val peek = r.peekBody(64).string().trimStart()
                peek.startsWith("{")
            }
        } catch (_: Exception) {
            false
        }
    }

    private fun getIpv4OnNetwork(cm: ConnectivityManager, network: Network): String? {
        val link = cm.getLinkProperties(network) ?: return null
        for (la in link.linkAddresses) {
            val a = la.address
            if (a is Inet4Address && !a.isLoopbackAddress) {
                return a.hostAddress
            }
        }
        return null
    }

    private fun candidateLastOctets(deviceLastOctet: Int): List<Int> {
        val base = listOf(1, 2, 3, 4, 5, 8, 10, 20, 30, 50, 60, 70, 80, 90, 200, 254) +
            (100..130).toList() +
            (131..200).toList() +
            (50..99 step 2).toList() +
            (150..210 step 5).toList()
        return (base + deviceLastOctet)
            .distinct()
            .filter { it in 1..254 }
            .sorted()
    }

    private fun candidateHostsHeuristic(ipv4: String): List<String> {
        val lastDot = ipv4.lastIndexOf('.')
        if (lastDot <= 0) return emptyList()
        val prefix = ipv4.substring(0, lastDot)
        val last = ipv4.substring(lastDot + 1).toIntOrNull() ?: 0
        return candidateLastOctets(last)
            .asSequence()
            .filter { it != last }
            .map { h -> "$prefix.$h" }
            .toList()
    }

    /** 同 /24 除本机外全部 IP，常见网关、DHCP 段略前置以更快命中 */
    private fun candidateHostsFullSubnet(ipv4: String): List<String> {
        val lastDot = ipv4.lastIndexOf('.')
        if (lastDot <= 0) return emptyList()
        val prefix = ipv4.substring(0, lastDot)
        val self = ipv4.substring(lastDot + 1).toIntOrNull() ?: return emptyList()
        val front = listOf(1, 254, 100, 101, 102, 103, 105, 108, 110, 120, 130, 150, 180, 200) +
            (2..20).toList() +
            (200..250).toList() +
            (20..99).toList() +
            (100..199).toList()
        val ordered = (front + (1..254).toList())
            .asSequence()
            .filter { it in 1..254 && it != self }
            .distinct()
            .map { o -> "$prefix.$o" }
            .toList()
        return ordered
    }

    private suspend fun scanCandidates(hosts: List<String>, client: OkHttpClient): String? = coroutineScope {
        for (chunk in hosts.chunked(FULL_SCAN_CONCURRENCY)) {
            val any = chunk.map { host ->
                async(Dispatchers.IO) {
                    if (isLanVideoServiceReachable("http://$host:$LAN_PORT", client)) {
                        "http://$host:$LAN_PORT/"
                    } else {
                        null
                    }
                }
            }.map { it.await() }
            any.firstOrNull { it != null }?.let { return@coroutineScope it }
        }
        null
    }

    /**
     * 先测当前已配置地址，失败则按本机同网段扫描常见宿主机
     * @param force 为 true 时跳过去抖，供首页在首次拉流失败/冷启动时强制重扫
     * @return 若新发现并切换了地址则返回新 baseUrl，否则 null（若已可用也不重复触发）
     */
    suspend fun discover(
        context: Context,
        cm: ConnectivityManager,
        network: Network,
        force: Boolean = false
    ): String? = withContext(Dispatchers.Default) {
        val now = System.currentTimeMillis()
        if (!force && now - lastProbeTimeMs < DEBOUNCE_MS) {
            return@withContext null
        }
        lastProbeTimeMs = now

        val appContext = context.applicationContext
        val current = NetworkModule.getBaseUrl()
        if (isLanVideoServiceReachable(current, probeClient)) {
            MyApplication.instance.setConnectionState(com.example.myapplication.ConnectionState.CONNECTED)
            return@withContext null
        }
        val ipv4 = getIpv4OnNetwork(cm, network) ?: return@withContext null
        MyApplication.instance.setConnectionState(com.example.myapplication.ConnectionState.SCANNING)
        val quick = candidateHostsHeuristic(ipv4)
        val found = scanCandidates(quick, fastProbeClient)
            ?: scanCandidates(candidateHostsFullSubnet(ipv4), fastProbeClient)
            // full subnet scan is limited to 3s max; caller may retry later
            ?: run {
                MyApplication.instance.setConnectionState(com.example.myapplication.ConnectionState.DISCONNECTED)
                return@withContext null
            }
        if (found != current) {
            NetworkModule.updateBaseUrl(found, appContext, notify = true)
            MyApplication.instance.setConnectionState(com.example.myapplication.ConnectionState.CONNECTED)
            return@withContext found
        }
        null
    }

    /**
     * 使用当前 [ConnectivityManager] 的 active network（用于冷启动时）
     * @param force 为 true 时跳过去抖（避免与 [MainActivity] 中首次探测在 1.5s 内冲突导致本次探测被空跑）
     */
    suspend fun discoverActiveNetwork(context: Context, force: Boolean = false): String? {
        val cm = context.applicationContext.getSystemService(ConnectivityManager::class.java) ?: return null
        val n = cm.activeNetwork ?: return null
        return discover(context, cm, n, force)
    }
}
