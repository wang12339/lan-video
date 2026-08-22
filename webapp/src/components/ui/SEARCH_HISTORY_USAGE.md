# SearchHistory 组件使用指南

## 概述

优化后的 SearchHistory 组件提供了增强的搜索历史管理功能，包括：

1. **本地存储优化** - 容量限制、数据验证、错误处理
2. **历史记录管理** - 去重、排序、过期清理
3. **清除功能** - 确认对话框、批量操作
4. **隐私模式** - 临时禁用历史记录、隐私模式切换

## 基本用法

### 1. 使用 SearchHistory 组件

```tsx
import SearchHistory from './components/ui/SearchHistory'
import { addToSearchHistory } from '../utils/searchHistory'

function SearchPage() {
  const [query, setQuery] = useState('')
  const [showHistory, setShowHistory] = useState(false)

  const handleSearch = (searchQuery: string) => {
    setQuery(searchQuery)
    addToSearchHistory(searchQuery)
    // 执行搜索逻辑
  }

  return (
    <div>
      <input
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onFocus={() => setShowHistory(true)}
        placeholder="搜索..."
      />
      
      <SearchHistory
        visible={showHistory}
        onSelect={handleSearch}
        showPrivacyToggle={true}
        showStats={true}
      />
    </div>
  )
}
```

### 2. 使用 SearchWithHistory 组件（推荐）

```tsx
import SearchWithHistory from './components/ui/SearchWithHistory'

function SearchPage() {
  const handleSearch = (query: string) => {
    console.log('搜索:', query)
    // 执行搜索逻辑
  }

  return (
    <SearchWithHistory
      placeholder="搜索视频..."
      onSearch={handleSearch}
      autoFocus={true}
    />
  )
}
```

## 功能详解

### 本地存储优化

- **容量限制**: 最多保存 30 条记录
- **数据验证**: 自动过滤无效记录（长度、时间戳等）
- **错误处理**: 存储异常时优雅降级
- **自动清理**: 过期记录自动删除（默认 30 天）

### 历史记录管理

- **智能去重**: 不区分大小写的去重，保留最新记录
- **时间排序**: 按时间倒序排列
- **批量操作**: 支持清空所有记录
- **统计信息**: 可选显示记录数量

### 清除功能

- **确认对话框**: 清空前显示确认提示
- **记录预览**: 显示将删除的记录数量
- **批量清除**: 一键清空所有历史
- **单条删除**: 支持删除单条记录

### 隐私模式

- **模式切换**: 一键开启/关闭隐私模式
- **即时生效**: 开启时立即清除现有记录
- **状态提示**: 显示隐私模式状态
- **持久化**: 模式状态本地保存

## API 参考

### SearchHistory 组件 Props

| 属性 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `visible` | `boolean` | - | 是否显示历史记录 |
| `onSelect` | `(query: string) => void` | - | 选择历史记录时的回调 |
| `showPrivacyToggle` | `boolean` | `true` | 是否显示隐私模式切换按钮 |
| `showStats` | `boolean` | `false` | 是否显示记录统计信息 |

### SearchWithHistory 组件 Props

| 属性 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `placeholder` | `string` | 搜索视频... | 输入框占位文本 |
| `onSearch` | `(query: string) => void` | - | 搜索提交回调 |
| `className` | `string` | `''` | 自定义样式类名 |
| `autoFocus` | `boolean` | `false` | 是否自动聚焦 |

### 工具函数

```typescript
// 搜索历史管理
import {
  getSearchHistory,        // 获取历史记录
  addToSearchHistory,      // 添加记录
  removeFromSearchHistory, // 删除单条记录
  clearSearchHistory,      // 清空所有记录
  isPrivacyMode,           // 检查隐私模式状态
  setPrivacyMode,          // 设置隐私模式
  cleanExpiredHistory,     // 清理过期记录
  getHistoryStats,         // 获取统计信息
  exportHistory,           // 导出历史记录
  importHistory            // 导入历史记录
} from '../utils/searchHistory'
```

## 最佳实践

### 1. 性能优化

- 使用 `useCallback` 包装事件处理函数
- 避免不必要的重渲染
- 合理使用 `useMemo` 缓存计算结果

### 2. 用户体验

- 提供视觉反馈（加载状态、确认对话框）
- 支持键盘操作（Enter 提交、Esc 关闭）
- 响应式设计，适配移动端

### 3. 数据安全

- 自动过滤无效数据
- 防止 XSS 攻击（输入验证）
- 隐私模式保护用户数据

### 4. 可访问性

- 正确的 ARIA 标签
- 键盘导航支持
- 屏幕阅读器友好

## 国际化

组件支持中英文两种语言，需要在 `locales` 目录下添加对应的翻译键：

```json
{
  "search": {
    "history": "搜索历史",
    "recentSearches": "最近搜索",
    "clear": "清空",
    "clearHistory": "清空搜索历史",
    "delete": "删除",
    "justNow": "刚刚",
    "minutesAgo": "{{count}} 分钟前",
    "hoursAgo": "{{count}} 小时前",
    "noHistory": "暂无搜索记录",
    "privacyModeOn": "隐私模式已开启",
    "privacyModeOff": "隐私模式已关闭",
    "enablePrivacy": "开启隐私模式",
    "disablePrivacy": "关闭隐私模式",
    "privacyModeActive": "隐私模式：搜索记录不会被保存",
    "confirmClearTitle": "确认清空",
    "confirmClearMessage": "确定要清空所有搜索历史吗？此操作不可撤销。",
    "recordsWillBeDeleted": "将删除 {{count}} 条记录",
    "clearAll": "全部清空"
  }
}
```

## 样式自定义

组件使用 CSS 变量，可以通过覆盖以下变量自定义样式：

```css
:root {
  --bg2: #1a1a1a;           /* 背景色 */
  --bg3: #2a2a2a;           /* 悬停背景色 */
  --border: #333;           /* 边框色 */
  --text: #fff;             /* 主文本色 */
  --text2: #ccc;            /* 次要文本色 */
  --text3: #888;            /* 辅助文本色 */
  --accent: #ff4433;        /* 强调色 */
  --accent-rgb: 255, 68, 51; /* 强调色 RGB 值 */
  --radius: 8px;            /* 圆角半径 */
  --radius-sm: 4px;         /* 小圆角半径 */
  --duration-fast: 0.15s;   /* 动画时长 */
}
```

## 常见问题

### Q: 历史记录不保存怎么办？

A: 检查是否开启了隐私模式，隐私模式下不会保存记录。

### Q: 如何限制历史记录数量？

A: 修改 `searchHistory.ts` 中的 `MAX_HISTORY` 常量。

### Q: 如何自定义过期时间？

A: 修改 `DEFAULT_EXPIRY_DAYS` 常量或调用 `cleanExpiredHistory(days)` 时传入自定义天数。

### Q: 如何导出用户的历史记录？

A: 使用 `exportHistory()` 函数返回 JSON 字符串。

### Q: 如何批量导入历史记录？

A: 使用 `importHistory(json)` 函数传入 JSON 字符串。

## 更新日志

### v2.0.0 (当前版本)

- ✅ 重构存储逻辑，提升性能
- ✅ 添加隐私模式支持
- ✅ 优化清除功能，增加确认对话框
- ✅ 支持数据导出/导入
- ✅ 改进错误处理和数据验证
- ✅ 增强可访问性支持
- ✅ 响应式设计优化

### v1.0.0

- 基础搜索历史功能
- 单条删除和清空
- 本地存储