# UI 重新设计 - 可爱卡哇伊风格

## [S1] 问题

当前 App 使用深色赛博朋克风格，用户希望完全重做 UI，采用可爱卡哇伊风格（Material You + 柔和粉彩配色）。

## [S2] 解决方案

使用 Jetpack Compose 重写所有 UI 层，采用以下设计规范：

### 设计风格

- **主题**: Material You (Material 3) + 可爱卡哇伊风格
- **配色**: 柔和粉彩 (樱花粉 #FFB5C5、天空蓝 #B5D8FF、奶油黄 #FFE5B5、薄荷绿 #B5FFD8、薰衣草 #E8D5F5)
- **背景**: 渐变 (#FFF0F5 → #F0F8FF → #FFF8DC)
- **卡片**: 大圆角 (20px)、毛玻璃效果、柔和彩色阴影
- **图标**: 超大可爱动物图标 (导航 48px、卡片 64px)

### 技术栈变更

**移除**:
- ViewBinding
- Fragment + Navigation Component
- XML 布局文件
- Material Components (XML)

**新增**:
- Jetpack Compose BOM
- Material 3 (Dynamic Colors)
- Compose Navigation
- Accompanist (系统栏)

### 项目结构

```
com.lanvideo.player/
├── ui/
│   ├── theme/
│   │   ├── Color.kt          # 粉彩色板
│   │   ├── Theme.kt          # Material 3 主题
│   │   └── Type.kt           # 字体
│   ├── navigation/
│   │   └── AppNavigation.kt  # Compose Navigation
│   ├── components/
│   │   ├── VideoCard.kt      # 视频卡片
│   │   ├── FeaturedCarousel.kt  # 推荐轮播
│   │   └── BottomNavBar.kt   # 底部导航
│   ├── home/
│   │   ├── HomeScreen.kt
│   │   └── HomeViewModel.kt
│   ├── player/
│   │   ├── PlayerScreen.kt
│   │   └── PlayerViewModel.kt
│   ├── search/
│   │   ├── SearchScreen.kt
│   │   └── SearchViewModel.kt
│   ├── user/
│   │   ├── UserScreen.kt
│   │   └── UserViewModel.kt
│   ├── history/
│   │   ├── HistoryScreen.kt
│   │   └── HistoryViewModel.kt
│   ├── settings/
│   │   └── SettingsScreen.kt
│   └── viewer/
│       └── ImageViewerScreen.kt
├── data/                 # 保留现有数据层
├── feature/              # 保留现有业务逻辑
└── MainActivity.kt       # 单 Activity
```

## [S3] 首页设计

### 布局结构

1. **状态栏**: 时间、信号、电池
2. **应用栏**: 毛玻璃效果，渐变标题
3. **导航卡片**: 3 个彩色圆角卡片 (首页/关注/发现)，48px 动物图标
4. **分类标签**: 圆角胶囊标签 (全部/视频/图片)
5. **视频网格**: 2 列布局，16px 间距
6. **底部导航**: 3 个标签 (消息/首页/我的)，中心图标 60px

### 视频卡片

- 高度: 140px 图片区 + 内容区
- 图片区: 64px 动物图标，渐变背景，爱心点赞数
- 内容区: 标题、时间、分类标签
- 阴影: 彩色柔和阴影

## [S4] 播放器设计

### 布局结构

1. **视频播放器**: 全屏，进度条
2. **视频信息**: 标题、上传时间、观看次数
3. **操作按钮**: 点赞、下载、分享
4. **相关推荐**: 横向滚动列表

### 功能

- ExoPlayer 视频播放
- 画中画 (PiP) 支持
- 手势控制 (亮度/音量/进度)
- 字幕支持

## [S5] 其他页面

### 搜索页

- 搜索栏: 圆角搜索框
- 搜索历史: 标签云
- 搜索结果: 视频网格

### 用户中心

- 用户头像: 圆形，彩色边框
- 用户信息: 昵称、签名
- 统计数据: 观看数、收藏数
- 功能列表: 设置、关于

### 设置页

- 分组列表: 服务器、账户、关于
- 开关组件: Material 3 Switch
- 列表项: 图标 + 标题 + 箭头

### 图片查看器

- 全屏查看
- 左右滑动
- 缩放手势

## [S6] 动画效果

- **页面过渡**: 柔和淡入淡出
- **点击反馈**: 弹性动画
- **悬浮效果**: 轻微放大
- **加载动画**: 可爱的加载指示器

## [S7] 测试策略

- **单元测试**: ViewModel 逻辑
- **UI 测试**: Compose 组件
- **集成测试**: 页面流程
- **手动测试**: 真机验证