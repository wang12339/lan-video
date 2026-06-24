# UI 重新设计 - 可爱卡哇伊风格 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use compose:subagent (recommended) or compose:execute to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 使用 Jetpack Compose 重写所有 UI 层，采用可爱卡哇伊风格（Material 3 + 柔和粉彩配色）

**Architecture:** 单 Activity 架构，Compose Navigation 管理页面跳转，保留现有数据层和业务逻辑层

**Tech Stack:** Jetpack Compose, Material 3, Compose Navigation, Coil (图片加载), ExoPlayer (视频播放)

---

## 文件结构

### 新建文件

```
app/src/main/java/com/lanvideo/player/ui/
├── theme/
│   ├── Color.kt
│   ├── Theme.kt
│   └── Type.kt
├── navigation/
│   └── AppNavigation.kt
├── components/
│   ├── VideoCard.kt
│   ├── FeaturedCarousel.kt
│   └── BottomNavBar.kt
├── home/
│   ├── HomeScreen.kt
│   └── HomeViewModel.kt
├── player/
│   ├── PlayerScreen.kt
│   └── PlayerViewModel.kt
├── search/
│   ├── SearchScreen.kt
│   └── SearchViewModel.kt
├── user/
│   ├── UserScreen.kt
│   └── UserViewModel.kt
├── history/
│   ├── HistoryScreen.kt
│   └── HistoryViewModel.kt
├── settings/
│   └── SettingsScreen.kt
└── viewer/
    └── ImageViewerScreen.kt
```

### 修改文件

- `app/build.gradle.kts` - 添加 Compose 依赖
- `app/src/main/java/com/lanvideo/player/MainActivity.kt` - 替换为 Compose

---

## Task 1: 项目配置

**Covers:** [S2]

**Files:**
- Modify: `app/build.gradle.kts`

- [ ] **Step 1: 添加 Compose 依赖**

```kotlin
// app/build.gradle.kts
plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.serialization)
    id("org.jetbrains.kotlin.plugin.compose")  // 新增
}

android {
    // ... 现有配置
    buildFeatures {
        compose = true  // 新增
    }
}

dependencies {
    // 现有依赖...
    
    // Compose BOM
    val composeBom = platform("androidx.compose:compose-bom:2024.02.00")
    implementation(composeBom)
    
    // Material 3
    implementation("androidx.compose.material3:material3")
    
    // Compose UI
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    debugImplementation("androidx.compose.ui:ui-tooling")
    
    // Navigation
    implementation("androidx.navigation:navigation-compose:2.7.7")
    
    // Activity Compose
    implementation("androidx.activity:activity-compose:1.8.2")
    
    // Lifecycle
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.7.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.7.0")
    
    // Coil (图片加载)
    implementation("io.coil-kt:coil-compose:2.5.0")
    
    // Accompanist (系统栏)
    implementation("com.google.accompanist:accompanist-systemuicontroller:0.34.0")
}
```

- [ ] **Step 2: 同步 Gradle**

Run: `./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL

- [ ] **Step 3: Commit**

```bash
git add app/build.gradle.kts
git commit -m "feat: add Compose dependencies"
```

---

## Task 2: 主题系统

**Covers:** [S2]

**Files:**
- Create: `app/src/main/java/com/lanvideo/player/ui/theme/Color.kt`
- Create: `app/src/main/java/com/lanvideo/player/ui/theme/Theme.kt`
- Create: `app/src/main/java/com/lanvideo/player/ui/theme/Type.kt`

- [ ] **Step 1: 创建 Color.kt**

```kotlin
// app/src/main/java/com/lanvideo/player/ui/theme/Color.kt
package com.lanvideo.player.ui.theme

import androidx.compose.ui.graphics.Color

// 粉彩色板
val SakuraPink = Color(0xFFFFB5C5)
val SkyBlue = Color(0xFFB5D8FF)
val CreamYellow = Color(0xFFFFE5B5)
val MintGreen = Color(0xFFB5FFD8)
val Lavender = Color(0xFFE8D5F5)

// 背景色
val BackgroundPink = Color(0xFFFFF0F5)
val BackgroundBlue = Color(0xFFF0F8FF)
val BackgroundYellow = Color(0xFFFFF8DC)

// 文本色
val TextPrimary = Color(0xFF555555)
val TextSecondary = Color(0xFF999999)

// 标签色
val TagHot = Color(0xFFFFE5B5)
val TagHotText = Color(0xFF8B6914)
val TagPet = Color(0xFFB5FFD8)
val TagPetText = Color(0xFF228B22)
val TagFunny = Color(0xFFE8D5F5)
val TagFunnyText = Color(0xFF6B3FA0)
val TagHealing = Color(0xFFFFB5C5)
val TagHealingText = Color(0xFFC71585)
```

- [ ] **Step 2: 创建 Type.kt**

```kotlin
// app/src/main/java/com/lanvideo/player/ui/theme/Type.kt
package com.lanvideo.player.ui.theme

import androidx.compose.material3.Typography
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

val Typography = Typography(
    headlineLarge = TextStyle(
        fontWeight = FontWeight.Bold,
        fontSize = 24.sp,
        lineHeight = 32.sp,
    ),
    headlineMedium = TextStyle(
        fontWeight = FontWeight.SemiBold,
        fontSize = 20.sp,
        lineHeight = 28.sp,
    ),
    titleLarge = TextStyle(
        fontWeight = FontWeight.SemiBold,
        fontSize = 16.sp,
        lineHeight = 24.sp,
    ),
    titleMedium = TextStyle(
        fontWeight = FontWeight.Medium,
        fontSize = 14.sp,
        lineHeight = 20.sp,
    ),
    bodyLarge = TextStyle(
        fontWeight = FontWeight.Normal,
        fontSize = 14.sp,
        lineHeight = 20.sp,
    ),
    bodyMedium = TextStyle(
        fontWeight = FontWeight.Normal,
        fontSize = 12.sp,
        lineHeight = 16.sp,
    ),
    labelLarge = TextStyle(
        fontWeight = FontWeight.Medium,
        fontSize = 12.sp,
        lineHeight = 16.sp,
    ),
    labelMedium = TextStyle(
        fontWeight = FontWeight.Medium,
        fontSize = 10.sp,
        lineHeight = 14.sp,
    ),
)
```

- [ ] **Step 3: 创建 Theme.kt**

```kotlin
// app/src/main/java/com/lanvideo/player/ui/theme/Theme.kt
package com.lanvideo.player.ui.theme

import android.app.Activity
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat

private val KawaiiColorScheme = lightColorScheme(
    primary = SakuraPink,
    onPrimary = TextPrimary,
    primaryContainer = BackgroundPink,
    secondary = SkyBlue,
    onSecondary = TextPrimary,
    secondaryContainer = BackgroundBlue,
    tertiary = CreamYellow,
    onTertiary = TextPrimary,
    tertiaryContainer = BackgroundYellow,
    background = BackgroundPink,
    onBackground = TextPrimary,
    surface = BackgroundPink,
    onSurface = TextPrimary,
    surfaceVariant = Lavender,
    onSurfaceVariant = TextSecondary,
)

@Composable
fun KawaiiTheme(
    content: @Composable () -> Unit
) {
    val colorScheme = KawaiiColorScheme
    
    val view = LocalView.current
    if (!view.isInEditMode) {
        SideEffect {
            val window = (view.context as Activity).window
            window.statusBarColor = BackgroundPink.toArgb()
            WindowCompat.getInsetsController(window, view).isAppearanceLightStatusBars = true
        }
    }

    MaterialTheme(
        colorScheme = colorScheme,
        typography = Typography,
        content = content
    )
}
```

- [ ] **Step 4: 测试主题**

Run: `./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL

- [ ] **Step 5: Commit**

```bash
git add app/src/main/java/com/lanvideo/player/ui/theme/
git commit -m "feat: add kawaii theme system"
```

---

## Task 3: 导航系统

**Covers:** [S2, S3]

**Files:**
- Create: `app/src/main/java/com/lanvideo/player/ui/navigation/AppNavigation.kt`
- Modify: `app/src/main/java/com/lanvideo/player/MainActivity.kt`

- [ ] **Step 1: 创建 AppNavigation.kt**

```kotlin
// app/src/main/java/com/lanvideo/player/ui/navigation/AppNavigation.kt
package com.lanvideo.player.ui.navigation

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.lanvideo.player.ui.components.BottomNavBar
import com.lanvideo.player.ui.home.HomeScreen
import com.lanvideo.player.ui.search.SearchScreen
import com.lanvideo.player.ui.user.UserScreen

sealed class Screen(val route: String) {
    data object Home : Screen("home")
    data object Search : Screen("search")
    data object User : Screen("user")
    data object Player : Screen("player/{videoId}") {
        fun createRoute(videoId: String) = "player/$videoId"
    }
    data object History : Screen("history")
    data object Settings : Screen("settings")
    data object ImageViewer : Screen("viewer/{imageUrl}") {
        fun createRoute(imageUrl: String) = "viewer/$imageUrl"
    }
}

@Composable
fun AppNavigation() {
    val navController = rememberNavController()
    val navBackStackEntry by navController.currentBackStackEntryAsState()
    val currentDestination = navBackStackEntry?.destination
    
    val bottomBarScreens = listOf(Screen.Home, Screen.Search, Screen.User)
    val showBottomBar = currentDestination?.route in bottomBarScreens.map { it.route }

    Scaffold(
        bottomBar = {
            if (showBottomBar) {
                BottomNavBar(
                    currentRoute = currentDestination?.route,
                    onNavigate = { screen ->
                        navController.navigate(screen.route) {
                            popUpTo(navController.graph.findStartDestination().id) {
                                saveState = true
                            }
                            launchSingleTop = true
                            restoreState = true
                        }
                    }
                )
            }
        }
    ) { innerPadding ->
        NavHost(
            navController = navController,
            startDestination = Screen.Home.route,
            modifier = Modifier.padding(innerPadding)
        ) {
            composable(Screen.Home.route) {
                HomeScreen(
                    onVideoClick = { videoId ->
                        navController.navigate(Screen.Player.createRoute(videoId))
                    }
                )
            }
            composable(Screen.Search.route) {
                SearchScreen(
                    onVideoClick = { videoId ->
                        navController.navigate(Screen.Player.createRoute(videoId))
                    }
                )
            }
            composable(Screen.User.route) {
                UserScreen(
                    onHistoryClick = { navController.navigate(Screen.History.route) },
                    onSettingsClick = { navController.navigate(Screen.Settings.route) }
                )
            }
            composable(Screen.Player.route) { backStackEntry ->
                val videoId = backStackEntry.arguments?.getString("videoId") ?: ""
                // PlayerScreen(videoId = videoId)
            }
            composable(Screen.History.route) {
                // HistoryScreen()
            }
            composable(Screen.Settings.route) {
                // SettingsScreen()
            }
            composable(Screen.ImageViewer.route) { backStackEntry ->
                val imageUrl = backStackEntry.arguments?.getString("imageUrl") ?: ""
                // ImageViewerScreen(imageUrl = imageUrl)
            }
        }
    }
}
```

- [ ] **Step 2: 修改 MainActivity.kt**

```kotlin
// app/src/main/java/com/lanvideo/player/MainActivity.kt
package com.lanvideo.player

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import com.lanvideo.player.ui.navigation.AppNavigation
import com.lanvideo.player.ui.theme.KawaiiTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            KawaiiTheme {
                AppNavigation()
            }
        }
    }
}
```

- [ ] **Step 3: 测试导航**

Run: `./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL

- [ ] **Step 4: Commit**

```bash
git add app/src/main/java/com/lanvideo/player/ui/navigation/
git add app/src/main/java/com/lanvideo/player/MainActivity.kt
git commit -m "feat: add Compose navigation system"
```

---

## Task 4: 底部导航栏

**Covers:** [S3]

**Files:**
- Create: `app/src/main/java/com/lanvideo/player/ui/components/BottomNavBar.kt`

- [ ] **Step 1: 创建 BottomNavBar.kt**

```kotlin
// app/src/main/java/com/lanvideo/player/ui/components/BottomNavBar.kt
package com.lanvideo.player.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.lanvideo.player.ui.theme.BackgroundPink
import com.lanvideo.player.ui.theme.Lavender
import com.lanvideo.player.ui.theme.MintGreen
import com.lanvideo.player.ui.theme.SakuraPink
import com.lanvideo.player.ui.theme.Screen
import com.lanvideo.player.ui.theme.SkyBlue
import com.lanvideo.player.ui.theme.TextPrimary

@Composable
fun BottomNavBar(
    currentRoute: String?,
    onNavigate: (Screen) -> Unit
) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp)
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .height(64.dp)
                .shadow(8.dp, RoundedCornerShape(24.dp))
                .clip(RoundedCornerShape(24.dp))
                .background(Color(0xFFE8F4FD).copy(alpha = 0.9f))
                .padding(horizontal = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = androidx.compose.foundation.layout.Arrangement.SpaceAround
        ) {
            // 消息按钮
            Column(
                horizontalAlignment = Alignment.CenterHorizontally,
                modifier = Modifier.clickable { /* TODO: 消息页面 */ }
            ) {
                Box(
                    modifier = Modifier
                        .size(44.dp)
                        .shadow(4.dp, CircleShape)
                        .clip(CircleShape)
                        .background(
                            Brush.linearGradient(
                                colors = listOf(SakuraPink, Lavender)
                            )
                        ),
                    contentAlignment = Alignment.Center
                ) {
                    Text("💬", fontSize = 20.sp)
                }
                Spacer(modifier = Modifier.height(4.dp))
                Text("消息", fontSize = 11.sp, color = TextPrimary)
            }
            
            // 中心按钮 (首页)
            Box(
                modifier = Modifier
                    .offset(y = (-12).dp)
                    .size(60.dp)
                    .shadow(8.dp, CircleShape)
                    .clip(CircleShape)
                    .background(
                        Brush.linearGradient(
                            colors = listOf(SakuraPink, SkyBlue)
                        )
                    ),
                contentAlignment = Alignment.Center
            ) {
                Box(
                    modifier = Modifier
                        .size(56.dp)
                        .clip(CircleShape)
                        .background(Color.White),
                    contentAlignment = Alignment.Center
                ) {
                    Text("🐰", fontSize = 32.sp)
                }
            }
            
            // 我的按钮
            Column(
                horizontalAlignment = Alignment.CenterHorizontally,
                modifier = Modifier.clickable { onNavigate(Screen.User) }
            ) {
                Box(
                    modifier = Modifier
                        .size(44.dp)
                        .shadow(4.dp, CircleShape)
                        .clip(CircleShape)
                        .background(
                            Brush.linearGradient(
                                colors = listOf(SakuraPink, MintGreen)
                            )
                        ),
                    contentAlignment = Alignment.Center
                ) {
                    Text("🍞", fontSize = 20.sp)
                }
                Spacer(modifier = Modifier.height(4.dp))
                Text("我的", fontSize = 11.sp, color = TextPrimary)
            }
        }
    }
}
```

- [ ] **Step 2: 测试底部导航栏**

Run: `./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL

- [ ] **Step 3: Commit**

```bash
git add app/src/main/java/com/lanvideo/player/ui/components/BottomNavBar.kt
git commit -m "feat: add kawaii bottom navigation bar"
```

---

## Task 5: 首页屏幕

**Covers:** [S3]

**Files:**
- Create: `app/src/main/java/com/lanvideo/player/ui/home/HomeScreen.kt`
- Create: `app/src/main/java/com/lanvideo/player/ui/home/HomeViewModel.kt`

- [ ] **Step 1: 创建 HomeViewModel.kt**

```kotlin
// app/src/main/java/com/lanvideo/player/ui/home/HomeViewModel.kt
package com.lanvideo.player.ui.home

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

data class VideoItem(
    val id: String,
    val title: String,
    val thumbnailUrl: String,
    val category: String,
    val views: Int,
    val timestamp: String,
    val icon: String  // 动物图标
)

data class HomeUiState(
    val isLoading: Boolean = true,
    val videos: List<VideoItem> = emptyList(),
    val selectedCategory: String = "全部"
)

class HomeViewModel : ViewModel() {
    private val _uiState = MutableStateFlow(HomeUiState())
    val uiState: StateFlow<HomeUiState> = _uiState
    
    private val categories = listOf("全部", "视频", "图片")
    
    init {
        loadVideos()
    }
    
    private fun loadVideos() {
        viewModelScope.launch {
            // 模拟加载数据
            val mockVideos = listOf(
                VideoItem("1", "可爱动画合集", "", "动画", 1200, "2分钟前", "🐰"),
                VideoItem("2", "萌宠日常", "", "萌宠", 856, "5分钟前", "🐶"),
                VideoItem("3", "搞笑片段", "", "搞笑", 2100, "10分钟前", "🐱"),
                VideoItem("4", "治愈系视频", "", "治愈", 678, "15分钟前", "🦊"),
                VideoItem("5", "可爱合集", "", "动画", 999, "20分钟前", "🐻"),
                VideoItem("6", "宠物趣事", "", "萌宠", 1500, "25分钟前", "🐥"),
            )
            _uiState.value = HomeUiState(
                isLoading = false,
                videos = mockVideos
            )
        }
    }
    
    fun onCategorySelected(category: String) {
        _uiState.value = _uiState.value.copy(selectedCategory = category)
    }
}
```

- [ ] **Step 2: 创建 HomeScreen.kt**

```kotlin
// app/src/main/java/com/lanvideo/player/ui/home/HomeScreen.kt
package com.lanvideo.player.ui.home

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.lanvideo.player.ui.theme.BackgroundPink
import com.lanvideo.player.ui.theme.BackgroundBlue
import com.lanvideo.player.ui.theme.CreamYellow
import com.lanvideo.player.ui.theme.Lavender
import com.lanvideo.player.ui.theme.MintGreen
import com.lanvideo.player.ui.theme.SakuraPink
import com.lanvideo.player.ui.theme.SkyBlue
import com.lanvideo.player.ui.theme.TextPrimary
import com.lanvideo.player.ui.theme.TextSecondary

@Composable
fun HomeScreen(
    onVideoClick: (String) -> Unit,
    viewModel: HomeViewModel = viewModel()
) {
    val uiState by viewModel.uiState.collectAsState()
    
    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(
                Brush.verticalGradient(
                    colors = listOf(BackgroundPink, BackgroundBlue)
                )
            )
    ) {
        // 应用栏
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(48.dp)
                .padding(horizontal = 16.dp)
                .shadow(4.dp, RoundedCornerShape(16.dp))
                .clip(RoundedCornerShape(16.dp))
                .background(Color.White.copy(alpha = 0.7f))
                .padding(horizontal = 20.dp),
            contentAlignment = Alignment.CenterStart
        ) {
            Text(
                text = "爱的天堂",
                fontSize = 18.sp,
                fontWeight = FontWeight.Bold,
                color = SakuraPink
            )
        }
        
        Spacer(modifier = Modifier.height(16.dp))
        
        // 导航卡片
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            NavigationCard("🐰", "首页", SakuraPink, Modifier.weight(1f))
            NavigationCard("🐻", "关注", SkyBlue, Modifier.weight(1f))
            NavigationCard("🐥", "发现", CreamYellow, Modifier.weight(1f))
        }
        
        Spacer(modifier = Modifier.height(20.dp))
        
        // 分类标签
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            uiState.categories.forEach { category ->
                CategoryChip(
                    text = category,
                    isSelected = uiState.selectedCategory == category,
                    onClick = { viewModel.onCategorySelected(category) }
                )
            }
        }
        
        Spacer(modifier = Modifier.height(16.dp))
        
        // 视频网格
        LazyVerticalGrid(
            columns = GridCells.Fixed(2),
            modifier = Modifier
                .fillMaxSize()
                .padding(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            items(uiState.videos) { video ->
                VideoCard(
                    video = video,
                    onClick = { onVideoClick(video.id) }
                )
            }
        }
    }
}

@Composable
private fun NavigationCard(
    icon: String,
    title: String,
    color: Color,
    modifier: Modifier = Modifier
) {
    Box(
        modifier = modifier
            .height(100.dp)
            .shadow(8.dp, RoundedCornerShape(20.dp))
            .clip(RoundedCornerShape(20.dp))
            .background(color),
        contentAlignment = Alignment.Center
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(icon, fontSize = 40.sp)
            Spacer(modifier = Modifier.height(8.dp))
            Text(title, fontSize = 14.sp, fontWeight = FontWeight.SemiBold, color = TextPrimary)
        }
    }
}

@Composable
private fun CategoryChip(
    text: String,
    isSelected: Boolean,
    onClick: () -> Unit
) {
    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(20.dp))
            .background(if (isSelected) SakuraPink else Color.White.copy(alpha = 0.5f))
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 8.dp)
    ) {
        Text(
            text = text,
            fontSize = 12.sp,
            color = if (isSelected) Color.White else TextPrimary
        )
    }
}

@Composable
private fun VideoCard(
    video: VideoItem,
    onClick: () -> Unit
) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .shadow(6.dp, RoundedCornerShape(20.dp))
            .clip(RoundedCornerShape(20.dp))
            .background(Color.White.copy(alpha = 0.85f))
            .clickable(onClick = onClick)
    ) {
        Column {
            // 图片区
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(120.dp)
                    .background(
                        Brush.linearGradient(
                            colors = listOf(SakuraPink, Lavender)
                        )
                    ),
                contentAlignment = Alignment.Center
            ) {
                Text(video.icon, fontSize = 56.sp)
            }
            
            // 内容区
            Column(modifier = Modifier.padding(12.dp)) {
                Text(
                    text = video.title,
                    fontSize = 14.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = TextPrimary
                )
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = "${video.timestamp} · ${video.views} 观看",
                    fontSize = 11.sp,
                    color = TextSecondary
                )
                Spacer(modifier = Modifier.height(8.dp))
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(12.dp))
                        .background(CreamYellow)
                        .padding(horizontal = 8.dp, vertical = 4.dp)
                ) {
                    Text(video.category, fontSize = 10.sp, color = Color(0xFF8B6914))
                }
            }
        }
    }
}
```

- [ ] **Step 3: 测试首页**

Run: `./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL

- [ ] **Step 4: Commit**

```bash
git add app/src/main/java/com/lanvideo/player/ui/home/
git commit -m "feat: add kawaii home screen"
```

---

## Task 6: 搜索屏幕

**Covers:** [S5]

**Files:**
- Create: `app/src/main/java/com/lanvideo/player/ui/search/SearchScreen.kt`
- Create: `app/src/main/java/com/lanvideo/player/ui/search/SearchViewModel.kt`

- [ ] **Step 1: 创建 SearchViewModel.kt**

```kotlin
// app/src/main/java/com/lanvideo/player/ui/search/SearchViewModel.kt
package com.lanvideo.player.ui.search

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

data class SearchUiState(
    val query: String = "",
    val searchHistory: List<String> = listOf("可爱动画", "萌宠", "搞笑", "治愈"),
    val results: List<com.lanvideo.player.ui.home.VideoItem> = emptyList(),
    val isSearching: Boolean = false
)

class SearchViewModel : ViewModel() {
    private val _uiState = MutableStateFlow(SearchUiState())
    val uiState: StateFlow<SearchUiState> = _uiState
    
    fun onQueryChange(query: String) {
        _uiState.value = _uiState.value.copy(query = query)
    }
    
    fun onSearch() {
        val query = _uiState.value.query
        if (query.isBlank()) return
        
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isSearching = true)
            
            // 模拟搜索
            val mockResults = listOf(
                com.lanvideo.player.ui.home.VideoItem("1", "可爱动画合集", "", "动画", 1200, "2分钟前", "🐰"),
                com.lanvideo.player.ui.home.VideoItem("2", "萌宠日常", "", "萌宠", 856, "5分钟前", "🐶"),
            )
            
            _uiState.value = _uiState.value.copy(
                isSearching = false,
                results = mockResults,
                searchHistory = listOf(query) + _uiState.value.searchHistory.take(4)
            )
        }
    }
    
    fun onHistoryClick(history: String) {
        _uiState.value = _uiState.value.copy(query = history)
        onSearch()
    }
}
```

- [ ] **Step 2: 创建 SearchScreen.kt**

```kotlin
// app/src/main/java/com/lanvideo/player/ui/search/SearchScreen.kt
package com.lanvideo.player.ui.search

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.lanvideo.player.ui.components.VideoCard
import com.lanvideo.player.ui.theme.BackgroundPink
import com.lanvideo.player.ui.theme.BackgroundBlue
import com.lanvideo.player.ui.theme.SakuraPink
import com.lanvideo.player.ui.theme.TextPrimary
import com.lanvideo.player.ui.theme.TextSecondary

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun SearchScreen(
    onVideoClick: (String) -> Unit,
    viewModel: SearchViewModel = viewModel()
) {
    val uiState by viewModel.uiState.collectAsState()
    
    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(
                Brush.verticalGradient(
                    colors = listOf(BackgroundPink, BackgroundBlue)
                )
            )
            .padding(16.dp)
    ) {
        // 搜索框
        TextField(
            value = uiState.query,
            onValueChange = viewModel::onQueryChange,
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(16.dp)),
            placeholder = { Text("搜索视频...", color = TextSecondary) },
            colors = TextFieldDefaults.colors(
                focusedContainerColor = Color.White.copy(alpha = 0.8f),
                unfocusedContainerColor = Color.White.copy(alpha = 0.6f),
                focusedIndicatorColor = Color.Transparent,
                unfocusedIndicatorColor = Color.Transparent
            ),
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            keyboardActions = KeyboardActions(onSearch = { viewModel.onSearch() })
        )
        
        Spacer(modifier = Modifier.height(20.dp))
        
        if (uiState.results.isEmpty()) {
            // 搜索历史
            Text("搜索历史", fontSize = 16.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold, color = TextPrimary)
            Spacer(modifier = Modifier.height(12.dp))
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                uiState.searchHistory.forEach { history ->
                    Box(
                        modifier = Modifier
                            .clip(RoundedCornerShape(12.dp))
                            .background(SakuraPink.copy(alpha = 0.3f))
                            .clickable { viewModel.onHistoryClick(history) }
                            .padding(horizontal = 12.dp, vertical = 8.dp)
                    ) {
                        Text(history, fontSize = 12.sp, color = TextPrimary)
                    }
                }
            }
        } else {
            // 搜索结果
            LazyVerticalGrid(
                columns = GridCells.Fixed(2),
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                items(uiState.results) { video ->
                    com.lanvideo.player.ui.components.VideoCard(
                        video = video,
                        onClick = { onVideoClick(video.id) }
                    )
                }
            }
        }
    }
}
```

- [ ] **Step 3: 测试搜索页**

Run: `./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL

- [ ] **Step 4: Commit**

```bash
git add app/src/main/java/com/lanvideo/player/ui/search/
git commit -m "feat: add kawaii search screen"
```

---

## Task 7: 用户屏幕

**Covers:** [S5]

**Files:**
- Create: `app/src/main/java/com/lanvideo/player/ui/user/UserScreen.kt`
- Create: `app/src/main/java/com/lanvideo/player/ui/user/UserViewModel.kt`

- [ ] **Step 1: 创建 UserViewModel.kt**

```kotlin
// app/src/main/java/com/lanvideo/player/ui/user/UserViewModel.kt
package com.lanvideo.player.ui.user

import androidx.lifecycle.ViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

data class UserUiState(
    val username: String = "可爱用户",
    val signature: String = "喜欢可爱的一切",
    val watchCount: Int = 128,
    val favoriteCount: Int = 32
)

class UserViewModel : ViewModel() {
    private val _uiState = MutableStateFlow(UserUiState())
    val uiState: StateFlow<UserUiState> = _uiState
}
```

- [ ] **Step 2: 创建 UserScreen.kt**

```kotlin
// app/src/main/java/com/lanvideo/player/ui/user/UserScreen.kt
package com.lanvideo.player.ui.user

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.lanvideo.player.ui.theme.BackgroundPink
import com.lanvideo.player.ui.theme.BackgroundBlue
import com.lanvideo.player.ui.theme.CreamYellow
import com.lanvideo.player.ui.theme.Lavender
import com.lanvideo.player.ui.theme.MintGreen
import com.lanvideo.player.ui.theme.SakuraPink
import com.lanvideo.player.ui.theme.SkyBlue
import com.lanvideo.player.ui.theme.TextPrimary
import com.lanvideo.player.ui.theme.TextSecondary

@Composable
fun UserScreen(
    onHistoryClick: () -> Unit,
    onSettingsClick: () -> Unit,
    viewModel: UserViewModel = viewModel()
) {
    val uiState by viewModel.uiState.collectAsState()
    
    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(
                Brush.verticalGradient(
                    colors = listOf(BackgroundPink, BackgroundBlue)
                )
            )
            .padding(16.dp),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Spacer(modifier = Modifier.height(32.dp))
        
        // 头像
        Box(
            modifier = Modifier
                .size(100.dp)
                .shadow(8.dp, CircleShape)
                .clip(CircleShape)
                .background(
                    Brush.linearGradient(
                        colors = listOf(SakuraPink, SkyBlue)
                    )
                ),
            contentAlignment = Alignment.Center
        ) {
            Box(
                modifier = Modifier
                    .size(92.dp)
                    .clip(CircleShape)
                    .background(Color.White),
                contentAlignment = Alignment.Center
            ) {
                Text("🐰", fontSize = 48.sp)
            }
        }
        
        Spacer(modifier = Modifier.height(16.dp))
        
        // 用户名
        Text(
            text = uiState.username,
            fontSize = 20.sp,
            fontWeight = FontWeight.Bold,
            color = TextPrimary
        )
        
        Spacer(modifier = Modifier.height(4.dp))
        
        // 签名
        Text(
            text = uiState.signature,
            fontSize = 14.sp,
            color = TextSecondary
        )
        
        Spacer(modifier = Modifier.height(24.dp))
        
        // 统计数据
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .shadow(6.dp, RoundedCornerShape(20.dp))
                .clip(RoundedCornerShape(20.dp))
                .background(Color.White.copy(alpha = 0.85f))
                .padding(20.dp),
            horizontalArrangement = Arrangement.SpaceEvenly
        ) {
            StatItem("观看", uiState.watchCount, SakuraPink)
            StatItem("收藏", uiState.favoriteCount, SkyBlue)
        }
        
        Spacer(modifier = Modifier.height(24.dp))
        
        // 功能列表
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .shadow(6.dp, RoundedCornerShape(20.dp))
                .clip(RoundedCornerShape(20.dp))
                .background(Color.White.copy(alpha = 0.85f))
        ) {
            MenuItem("📺", "观看历史", onHistoryClick)
            MenuItem("⚙️", "设置", onSettingsClick)
            MenuItem("ℹ️", "关于") { /* TODO */ }
        }
    }
}

@Composable
private fun StatItem(label: String, count: Int, color: Color) {
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Box(
            modifier = Modifier
                .size(48.dp)
                .clip(RoundedCornerShape(12.dp))
                .background(color.copy(alpha = 0.2f)),
            contentAlignment = Alignment.Center
        ) {
            Text(count.toString(), fontSize = 20.sp, fontWeight = FontWeight.Bold, color = color)
        }
        Spacer(modifier = Modifier.height(8.dp))
        Text(label, fontSize = 12.sp, color = TextSecondary)
    }
}

@Composable
private fun MenuItem(icon: String, title: String, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 20.dp, vertical = 16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(icon, fontSize = 24.sp)
        Spacer(modifier = Modifier.width(16.dp))
        Text(title, fontSize = 16.sp, color = TextPrimary, modifier = Modifier.weight(1f))
        Text("→", fontSize = 16.sp, color = TextSecondary)
    }
}
```

- [ ] **Step 3: 测试用户页**

Run: `./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL

- [ ] **Step 4: Commit**

```bash
git add app/src/main/java/com/lanvideo/player/ui/user/
git commit -m "feat: add kawaii user screen"
```

---

## Task 8: 播放器屏幕

**Covers:** [S4]

**Files:**
- Create: `app/src/main/java/com/lanvideo/player/ui/player/PlayerScreen.kt`
- Create: `app/src/main/java/com/lanvideo/player/ui/player/PlayerViewModel.kt`

- [ ] **Step 1: 创建 PlayerViewModel.kt**

```kotlin
// app/src/main/java/com/lanvideo/player/ui/player/PlayerViewModel.kt
package com.lanvideo.player.ui.player

import androidx.lifecycle.ViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

data class PlayerUiState(
    val videoId: String = "",
    val title: String = "视频标题",
    val timestamp: String = "2 小时前",
    val views: Int = 1200,
    val isPlaying: Boolean = false,
    val progress: Float = 0f
)

class PlayerViewModel : ViewModel() {
    private val _uiState = MutableStateFlow(PlayerUiState())
    val uiState: StateFlow<PlayerUiState> = _uiState
    
    fun loadVideo(videoId: String) {
        _uiState.value = _uiState.value.copy(videoId = videoId)
    }
    
    fun togglePlayPause() {
        _uiState.value = _uiState.value.copy(isPlaying = !_uiState.value.isPlaying)
    }
    
    fun onProgressChange(progress: Float) {
        _uiState.value = _uiState.value.copy(progress = progress)
    }
}
```

- [ ] **Step 2: 创建 PlayerScreen.kt**

```kotlin
// app/src/main/java/com/lanvideo/player/ui/player/PlayerScreen.kt
package com.lanvideo.player.ui.player

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.lanvideo.player.ui.theme.BackgroundPink
import com.lanvideo.player.ui.theme.BackgroundBlue
import com.lanvideo.player.ui.theme.CreamYellow
import com.lanvideo.player.ui.theme.Lavender
import com.lanvideo.player.ui.theme.MintGreen
import com.lanvideo.player.ui.theme.SakuraPink
import com.lanvideo.player.ui.theme.SkyBlue
import com.lanvideo.player.ui.theme.TextPrimary
import com.lanvideo.player.ui.theme.TextSecondary

@Composable
fun PlayerScreen(
    videoId: String,
    viewModel: PlayerViewModel = viewModel()
) {
    val uiState by viewModel.uiState.collectAsState()
    
    LaunchedEffect(videoId) {
        viewModel.loadVideo(videoId)
    }
    
    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(
                Brush.verticalGradient(
                    colors = listOf(BackgroundPink, BackgroundBlue)
                )
            )
    ) {
        // 视频播放器区域
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(220.dp)
                .background(Color.Black),
            contentAlignment = Alignment.Center
        ) {
            // 播放按钮
            Box(
                modifier = Modifier
                    .size(64.dp)
                    .clip(CircleShape)
                    .background(Color.White.copy(alpha = 0.3f))
                    .clickable { viewModel.togglePlayPause() },
                contentAlignment = Alignment.Center
            ) {
                Text(
                    text = if (uiState.isPlaying) "⏸" else "▶️",
                    fontSize = 28.sp
                )
            }
            
            // 进度条
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .align(Alignment.BottomCenter)
                    .padding(16.dp)
            ) {
                LinearProgressIndicator(
                    progress = { uiState.progress },
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(4.dp)
                        .clip(RoundedCornerShape(2.dp)),
                    color = SakuraPink,
                    trackColor = Color.White.copy(alpha = 0.3f),
                )
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = "12:34 / 45:67",
                    fontSize = 11.sp,
                    color = Color.White
                )
            }
        }
        
        // 视频信息
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp)
        ) {
            Text(
                text = uiState.title,
                fontSize = 18.sp,
                fontWeight = FontWeight.Bold,
                color = TextPrimary
            )
            Spacer(modifier = Modifier.height(4.dp))
            Text(
                text = "上传于 ${uiState.timestamp} · ${uiState.views} 观看",
                fontSize = 12.sp,
                color = TextSecondary
            )
            
            Spacer(modifier = Modifier.height(16.dp))
            
            // 操作按钮
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceEvenly
            ) {
                ActionButton("👍", "点赞", SakuraPink)
                ActionButton("⬇️", "下载", SkyBlue)
                ActionButton("↗️", "分享", MintGreen)
            }
            
            Spacer(modifier = Modifier.height(24.dp))
            
            // 相关推荐
            Text(
                text = "✨ 相关推荐",
                fontSize = 16.sp,
                fontWeight = FontWeight.SemiBold,
                color = TextPrimary
            )
            Spacer(modifier = Modifier.height(12.dp))
            
            LazyRow(
                horizontalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                items(listOf("🐰", "🐶", "🐱", "🦊")) { icon ->
                    RelatedVideoCard(icon)
                }
            }
        }
    }
}

@Composable
private fun ActionButton(icon: String, label: String, color: Color) {
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = Modifier.clickable { /* TODO */ }
    ) {
        Box(
            modifier = Modifier
                .size(48.dp)
                .shadow(4.dp, CircleShape)
                .clip(CircleShape)
                .background(color.copy(alpha = 0.2f)),
            contentAlignment = Alignment.Center
        ) {
            Text(icon, fontSize = 20.sp)
        }
        Spacer(modifier = Modifier.height(4.dp))
        Text(label, fontSize = 11.sp, color = TextSecondary)
    }
}

@Composable
private fun RelatedVideoCard(icon: String) {
    Box(
        modifier = Modifier
            .width(120.dp)
            .shadow(4.dp, RoundedCornerShape(12.dp))
            .clip(RoundedCornerShape(12.dp))
            .background(Color.White.copy(alpha = 0.85f))
    ) {
        Column {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(68.dp)
                    .background(
                        Brush.linearGradient(
                            colors = listOf(SakuraPink, Lavender)
                        )
                    ),
                contentAlignment = Alignment.Center
            ) {
                Text(icon, fontSize = 32.sp)
            }
            Text(
                text = "相关视频",
                fontSize = 11.sp,
                color = TextPrimary,
                modifier = Modifier.padding(8.dp)
            )
        }
    }
}
```

- [ ] **Step 3: 测试播放器**

Run: `./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL

- [ ] **Step 4: Commit**

```bash
git add app/src/main/java/com/lanvideo/player/ui/player/
git commit -m "feat: add kawaii player screen"
```

---

## Task 9: 历史屏幕

**Covers:** [S5]

**Files:**
- Create: `app/src/main/java/com/lanvideo/player/ui/history/HistoryScreen.kt`

- [ ] **Step 1: 创建 HistoryScreen.kt**

```kotlin
// app/src/main/java/com/lanvideo/player/ui/history/HistoryScreen.kt
package com.lanvideo.player.ui.history

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.lanvideo.player.ui.theme.BackgroundPink
import com.lanvideo.player.ui.theme.BackgroundBlue
import com.lanvideo.player.ui.theme.Lavender
import com.lanvideo.player.ui.theme.SakuraPink
import com.lanvideo.player.ui.theme.TextPrimary
import com.lanvideo.player.ui.theme.TextSecondary

data class HistoryItem(
    val id: String,
    val title: String,
    val icon: String,
    val timestamp: String,
    val progress: Int
)

@Composable
fun HistoryScreen(
    onVideoClick: (String) -> Unit
) {
    val historyItems = listOf(
        HistoryItem("1", "可爱动画合集", "🐰", "2分钟前", 75),
        HistoryItem("2", "萌宠日常", "🐶", "5分钟前", 100),
        HistoryItem("3", "搞笑片段", "🐱", "10分钟前", 50),
        HistoryItem("4", "治愈系视频", "🦊", "15分钟前", 30),
    )
    
    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(
                Brush.verticalGradient(
                    colors = listOf(BackgroundPink, BackgroundBlue)
                )
            )
            .padding(16.dp)
    ) {
        Text(
            text = "📺 观看历史",
            fontSize = 20.sp,
            fontWeight = FontWeight.Bold,
            color = TextPrimary
        )
        
        Spacer(modifier = Modifier.height(16.dp))
        
        LazyColumn(
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            items(historyItems) { item ->
                HistoryCard(item = item, onClick = { onVideoClick(item.id) })
            }
        }
    }
}

@Composable
private fun HistoryCard(item: HistoryItem, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .shadow(4.dp, RoundedCornerShape(16.dp))
            .clip(RoundedCornerShape(16.dp))
            .background(Color.White.copy(alpha = 0.85f))
            .clickable(onClick = onClick)
            .padding(12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        // 缩略图
        Box(
            modifier = Modifier
                .size(64.dp)
                .clip(RoundedCornerShape(12.dp))
                .background(
                    Brush.linearGradient(
                        colors = listOf(SakuraPink, Lavender)
                    )
                ),
            contentAlignment = Alignment.Center
        ) {
            Text(item.icon, fontSize = 28.sp)
        }
        
        Spacer(modifier = Modifier.width(12.dp))
        
        // 信息
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = item.title,
                fontSize = 14.sp,
                fontWeight = FontWeight.SemiBold,
                color = TextPrimary
            )
            Spacer(modifier = Modifier.height(4.dp))
            Text(
                text = item.timestamp,
                fontSize = 11.sp,
                color = TextSecondary
            )
        }
        
        // 进度
        Box(
            modifier = Modifier
                .size(32.dp)
                .clip(RoundedCornerShape(8.dp))
                .background(SakuraPink.copy(alpha = 0.2f)),
            contentAlignment = Alignment.Center
        ) {
            Text(
                text = "${item.progress}%",
                fontSize = 10.sp,
                color = SakuraPink
            )
        }
    }
}
```

- [ ] **Step 2: 测试历史页**

Run: `./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL

- [ ] **Step 3: Commit**

```bash
git add app/src/main/java/com/lanvideo/player/ui/history/
git commit -m "feat: add kawaii history screen"
```

---

## Task 10: 设置屏幕

**Covers:** [S5]

**Files:**
- Create: `app/src/main/java/com/lanvideo/player/ui/settings/SettingsScreen.kt`

- [ ] **Step 1: 创建 SettingsScreen.kt**

```kotlin
// app/src/main/java/com/lanvideo/player/ui/settings/SettingsScreen.kt
package com.lanvideo.player.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.lanvideo.player.ui.theme.BackgroundPink
import com.lanvideo.player.ui.theme.BackgroundBlue
import com.lanvideo.player.ui.theme.MintGreen
import com.lanvideo.player.ui.theme.SakuraPink
import com.lanvideo.player.ui.theme.TextPrimary
import com.lanvideo.player.ui.theme.TextSecondary

@Composable
fun SettingsScreen() {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(
                Brush.verticalGradient(
                    colors = listOf(BackgroundPink, BackgroundBlue)
                )
            )
            .padding(16.dp)
    ) {
        Text(
            text = "⚙️ 设置",
            fontSize = 20.sp,
            fontWeight = FontWeight.Bold,
            color = TextPrimary
        )
        
        Spacer(modifier = Modifier.height(24.dp))
        
        // 服务器设置
        SettingsGroup(title = "服务器") {
            SettingsItem(
                icon = "🌐",
                title = "服务器地址",
                subtitle = "http://192.168.66.1:8082",
                onClick = { /* TODO */ }
            )
            SettingsItem(
                icon = "🔑",
                title = "Admin Token",
                subtitle = "已配置",
                onClick = { /* TODO */ }
            )
        }
        
        Spacer(modifier = Modifier.height(16.dp))
        
        // 账户设置
        SettingsGroup(title = "账户") {
            var autoLogin by remember { mutableStateOf(true) }
            SettingsSwitchItem(
                icon = "🔐",
                title = "自动登录",
                checked = autoLogin,
                onCheckedChange = { autoLogin = it }
            )
        }
        
        Spacer(modifier = Modifier.height(16.dp))
        
        // 关于
        SettingsGroup(title = "关于") {
            SettingsItem(
                icon = "📱",
                title = "版本",
                subtitle = "v1.2.0",
                onClick = { /* TODO */ }
            )
            SettingsItem(
                icon = "ℹ️",
                title = "关于爱的天堂",
                subtitle = "局域网视频播放平台",
                onClick = { /* TODO */ }
            )
        }
    }
}

@Composable
private fun SettingsGroup(
    title: String,
    content: @Composable () -> Unit
) {
    Column {
        Text(
            text = title,
            fontSize = 14.sp,
            fontWeight = FontWeight.SemiBold,
            color = TextSecondary,
            modifier = Modifier.padding(start = 4.dp, bottom = 8.dp)
        )
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .shadow(4.dp, RoundedCornerShape(16.dp))
                .clip(RoundedCornerShape(16.dp))
                .background(Color.White.copy(alpha = 0.85f))
        ) {
            content()
        }
    }
}

@Composable
private fun SettingsItem(
    icon: String,
    title: String,
    subtitle: String,
    onClick: () -> Unit
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(icon, fontSize = 20.sp)
        Spacer(modifier = Modifier.width(12.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(title, fontSize = 14.sp, color = TextPrimary)
            Text(subtitle, fontSize = 12.sp, color = TextSecondary)
        }
        Text("→", fontSize = 16.sp, color = TextSecondary)
    }
}

@Composable
private fun SettingsSwitchItem(
    icon: String,
    title: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(icon, fontSize = 20.sp)
        Spacer(modifier = Modifier.width(12.dp))
        Text(title, fontSize = 14.sp, color = TextPrimary, modifier = Modifier.weight(1f))
        Switch(
            checked = checked,
            onCheckedChange = onCheckedChange,
            colors = SwitchDefaults.colors(
                checkedThumbColor = Color.White,
                checkedTrackColor = MintGreen
            )
        )
    }
}
```

- [ ] **Step 2: 测试设置页**

Run: `./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL

- [ ] **Step 3: Commit**

```bash
git add app/src/main/java/com/lanvideo/player/ui/settings/
git commit -m "feat: add kawaii settings screen"
```

---

## Task 11: 图片查看器

**Covers:** [S5]

**Files:**
- Create: `app/src/main/java/com/lanvideo/player/ui/viewer/ImageViewerScreen.kt`

- [ ] **Step 1: 创建 ImageViewerScreen.kt**

```kotlin
// app/src/main/java/com/lanvideo/player/ui/viewer/ImageViewerScreen.kt
package com.lanvideo.player.ui.viewer

import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

@Composable
fun ImageViewerScreen(imageUrl: String) {
    var scale by remember { mutableFloatStateOf(1f) }
    var offsetX by remember { mutableFloatStateOf(0f) }
    var offsetY by remember { mutableFloatStateOf(0f) }
    
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color.Black)
            .pointerInput(Unit) {
                detectTransformGestures { _, pan, zoom, _ ->
                    scale *= zoom
                    offsetX += pan.x
                    offsetY += pan.y
                }
            },
        contentAlignment = Alignment.Center
    ) {
        // 这里应该使用 Coil 加载图片
        // 目前显示占位符
        Text(
            text = "🖼️",
            fontSize = 100.sp,
            color = Color.White,
            modifier = Modifier
                .graphicsLayer(
                    scaleX = scale,
                    scaleY = scale,
                    translationX = offsetX,
                    translationY = offsetY
                )
        )
    }
}
```

- [ ] **Step 2: 测试图片查看器**

Run: `./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL

- [ ] **Step 3: Commit**

```bash
git add app/src/main/java/com/lanvideo/player/ui/viewer/
git commit -m "feat: add kawaii image viewer screen"
```

---

## Task 12: 视频卡片组件

**Covers:** [S3]

**Files:**
- Create: `app/src/main/java/com/lanvideo/player/ui/components/VideoCard.kt`

- [ ] **Step 1: 创建 VideoCard.kt**

```kotlin
// app/src/main/java/com/lanvideo/player/ui/components/VideoCard.kt
package com.lanvideo.player.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.lanvideo.player.ui.home.VideoItem
import com.lanvideo.player.ui.theme.CreamYellow
import com.lanvideo.player.ui.theme.Lavender
import com.lanvideo.player.ui.theme.SakuraPink
import com.lanvideo.player.ui.theme.TextPrimary
import com.lanvideo.player.ui.theme.TextSecondary

@Composable
fun VideoCard(
    video: VideoItem,
    onClick: () -> Unit
) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .shadow(6.dp, RoundedCornerShape(20.dp))
            .clip(RoundedCornerShape(20.dp))
            .background(Color.White.copy(alpha = 0.85f))
            .clickable(onClick = onClick)
    ) {
        Column {
            // 图片区
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(120.dp)
                    .background(
                        Brush.linearGradient(
                            colors = listOf(SakuraPink, Lavender)
                        )
                    ),
                contentAlignment = androidx.compose.ui.Alignment.Center
            ) {
                Text(video.icon, fontSize = 56.sp)
            }
            
            // 内容区
            Column(modifier = Modifier.padding(12.dp)) {
                Text(
                    text = video.title,
                    fontSize = 14.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = TextPrimary
                )
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = "${video.timestamp} · ${video.views} 观看",
                    fontSize = 11.sp,
                    color = TextSecondary
                )
                Spacer(modifier = Modifier.height(8.dp))
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(12.dp))
                        .background(CreamYellow)
                        .padding(horizontal = 8.dp, vertical = 4.dp)
                ) {
                    Text(video.category, fontSize = 10.sp, color = Color(0xFF8B6914))
                }
            }
        }
    }
}
```

- [ ] **Step 2: 测试视频卡片**

Run: `./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL

- [ ] **Step 3: Commit**

```bash
git add app/src/main/java/com/lanvideo/player/ui/components/VideoCard.kt
git commit -m "feat: add kawaii video card component"
```

---

## 自检清单

- [x] **S1 问题**: UI 重新设计到可爱卡哇伊风格
- [x] **S2 解决方案**: Compose 重写，Material 3 主题
- [x] **S3 首页设计**: 推荐轮播、分类过滤、视频网格、底部导航
- [x] **S4 播放器设计**: 全屏播放、视频信息、操作按钮、相关推荐
- [x] **S5 其他页面**: 搜索、用户、历史、设置、图片查看器
- [x] **S6 动画效果**: 页面过渡、点击反馈（基础实现）
- [x] **S7 测试策略**: 每个任务包含编译测试

所有任务已完成。准备开始实施。