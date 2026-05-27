# ============================================================
# atmos-android ProGuard / R8 规则
# ============================================================

# ---- 保留行号（用于 Sentry 崩溃追踪） ----
-keepattributes SourceFile,LineNumberTable
-renamesourcefileattribute SourceFile

# ---- Kotlin Serialization ----
# @Serializable 类在编译期生成序列化器，不能被混淆
-keepattributes *Annotation*, RuntimeVisibleAnnotations
-keepclassmembers class kotlinx.serialization.json.** { *; }
-keep,includedescriptorclasses class com.lanvideo.player.**$$serializer { *; }
-keepclassmembers class com.lanvideo.player.** {
    *** Companion;
}
-keepclasseswithmembers class com.lanvideo.player.** {
    kotlinx.serialization.KSerializer serializer(...);
}

# ---- Retrofit / OkHttp ----
-keep,allowobfuscation,allowshrinking interface retrofit2.Call
-keep,allowobfuscation,allowshrinking class retrofit2.Response
-keep,allowobfuscation,allowshrinking class kotlin.coroutines.Continuation
-keepclassmembers,allowshrinking,allowobfuscation interface * {
    @retrofit2.http.* <methods>;
}
-dontwarn org.codehaus.mojo.animal_sniffer.IgnoreJRERequirement
-dontwarn javax.annotation.**
-dontwarn kotlin.Unit
-dontwarn okhttp3.internal.platform.**

# ---- ExoPlayer / Media3 ----
-keep class androidx.media3.** { *; }
-dontwarn androidx.media3.**

# ---- Coil ----
-keep class coil.** { *; }
-dontwarn coil.**

# ---- Kotlin Coroutines ----
-keepnames class kotlinx.coroutines.internal.MainDispatcherFactory {}
-keepnames class kotlinx.coroutines.CoroutineExceptionHandler {}
-keepclassmembers class kotlinx.coroutines.** {
    volatile <fields>;
}

# ---- 自定义数据模型（所有 data class API 响应） ----
-keep class com.lanvideo.player.data.model.** { *; }
-keep class com.lanvideo.player.data.network.** { *; }
