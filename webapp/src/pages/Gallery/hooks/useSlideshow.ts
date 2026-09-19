import { useEffect, useRef, useState, useCallback, useMemo } from 'react'

/**
 * 幻灯片播放 Hook
 * - setInterval 驱动自动翻页，卸载时清理定时器
 * - shuffle: 随机不重复序列（Fisher-Yates）；序列随 total/shuffle 变化重建
 * - loop: 非循环模式到达末尾自动暂停
 * - visibilitychange: 页面隐藏时自动暂停
 * - 键盘空格切播放/暂停由外部监听处理，hook 仅暴露 toggle
 */
export interface UseSlideshowOptions {
  /** 总图片数 */
  total: number
  /** 当前索引（外部控制） */
  index: number
  /** 索引变更回调（外部调用 setLbIndex） */
  onIndexChange: (idx: number) => void
  /** 间隔毫秒 */
  intervalMs: number
  /** 随机播放 */
  shuffle: boolean
  /** 循环播放 */
  loop: boolean
}

export interface UseSlideshowReturn {
  /** 是否正在播放 */
  playing: boolean
  /** 开始播放 */
  play: () => void
  /** 暂停播放 */
  pause: () => void
  /** 切换播放/暂停 */
  toggle: () => void
  /** 下一张 */
  next: () => void
  /** 上一张 */
  prev: () => void
  /** 当前间隔内的进度 0-1（用于进度条） */
  progress: number
  /** 当前播放序列位置（shuffle 时为打乱后的位置） */
  sequenceIndex: number
  /** 序列中的下一张实际索引（供预加载） */
  nextIndex: number
}

/** 生成不重复的随机序列（Fisher-Yates 洗牌） */
function generateShuffledSequence(length: number): number[] {
  const arr = Array.from({ length }, (_, i) => i)
  for (let i = length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1))
    const tmp = arr[i]!
    arr[i] = arr[j]!
    arr[j] = tmp
  }
  return arr
}

export function useSlideshow({
  total,
  index,
  onIndexChange,
  intervalMs,
  shuffle,
  loop,
}: UseSlideshowOptions): UseSlideshowReturn {
  const [playing, setPlaying] = useState(false)
  const [progress, setProgress] = useState(0)
  const [sequenceIndex, setSequenceIndex] = useState(0)

  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const startedAtRef = useRef(0)
  const sequenceRef = useRef<number[]>([])

  // 播放序列：非随机时即自然顺序
  const currentSequence = useMemo(() => {
    if (!shuffle) return Array.from({ length: total }, (_, i) => i)
    if (sequenceRef.current.length !== total) {
      sequenceRef.current = generateShuffledSequence(total)
    }
    return sequenceRef.current
  }, [shuffle, total])

  const getIndexFromSequence = useCallback(
    (seqIdx: number): number => (shuffle ? currentSequence[seqIdx] ?? 0 : seqIdx),
    [shuffle, currentSequence]
  )

  const findSequenceIndex = useCallback(
    (idx: number): number => (shuffle ? Math.max(0, currentSequence.indexOf(idx)) : idx),
    [shuffle, currentSequence]
  )

  const clearTimer = useCallback(() => {
    if (timerRef.current) {
      clearInterval(timerRef.current)
      timerRef.current = null
    }
  }, [])

  const pause = useCallback(() => setPlaying(false), [])

  // 步进到序列中的下一张/上一张；非循环到达末尾时暂停
  const step = useCallback(
    (direction: 1 | -1) => {
      if (total === 0) return
      let nextSeq = sequenceIndex + direction
      if (nextSeq >= total) {
        if (!loop) {
          setPlaying(false)
          return
        }
        nextSeq = 0
      } else if (nextSeq < 0) {
        nextSeq = loop ? total - 1 : 0
      }
      setSequenceIndex(nextSeq)
      onIndexChange(getIndexFromSequence(nextSeq))
      startedAtRef.current = Date.now()
      setProgress(0)
    },
    [total, loop, sequenceIndex, getIndexFromSequence, onIndexChange]
  )

  const next = useCallback(() => step(1), [step])
  const prev = useCallback(() => step(-1), [step])

  const play = useCallback(() => {
    if (total === 0) return
    // 非循环时在末尾重新开始
    if (!loop && sequenceIndex >= total - 1) {
      onIndexChange(getIndexFromSequence(0))
      setSequenceIndex(0)
    }
    startedAtRef.current = Date.now()
    setProgress(0)
    setPlaying(true)
  }, [total, loop, sequenceIndex, getIndexFromSequence, onIndexChange])

  const toggle = useCallback(() => {
    if (playing) pause()
    else play()
  }, [playing, play, pause])

  // 外部索引变化（手动点击导航/键盘）时同步序列位置并重置计时
  useEffect(() => {
    const seq = findSequenceIndex(index)
    if (seq !== sequenceIndex) {
      setSequenceIndex(seq)
      if (playing) {
        startedAtRef.current = Date.now()
        setProgress(0)
      }
    }
  }, [index, findSequenceIndex, sequenceIndex, playing])

  // 定时器：每 50ms 刷新进度，到达间隔后步进
  const stepRef = useRef(step)
  useEffect(() => {
    stepRef.current = step
  }, [step])

  useEffect(() => {
    if (!playing) return
    startedAtRef.current = Date.now()
    setProgress(0)
    timerRef.current = setInterval(() => {
      const p = Math.min((Date.now() - startedAtRef.current) / intervalMs, 1)
      setProgress(p)
      if (p >= 1) {
        startedAtRef.current = Date.now()
        setProgress(0)
        stepRef.current(1)
      }
    }, 50)
    return clearTimer
  }, [playing, intervalMs, clearTimer])

  // 页面隐藏时暂停（避免后台空转）
  useEffect(() => {
    const onVisibilityChange = () => {
      if (document.hidden) setPlaying(false)
    }
    document.addEventListener('visibilitychange', onVisibilityChange)
    return () => document.removeEventListener('visibilitychange', onVisibilityChange)
  }, [])

  const nextIndex = total > 0 ? getIndexFromSequence((sequenceIndex + 1) % total) : 0

  return {
    playing,
    play,
    pause,
    toggle,
    next,
    prev,
    progress,
    sequenceIndex,
    nextIndex,
  }
}
