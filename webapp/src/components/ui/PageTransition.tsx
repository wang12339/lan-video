import { useEffect, useRef, useState, memo } from 'react'
import './PageTransition.css'

interface Props {
  children: React.ReactNode
  /** Unique key that triggers the transition (e.g. pathname) */
  transitionKey: string
}

function PageTransitionImpl({ children, transitionKey }: Props) {
  const [state, setState] = useState<'entering' | 'entered' | 'exiting'>('entered')
  const [displayChildren, setDisplayChildren] = useState(children)
  const prevKeyRef = useRef(transitionKey)
  const exitTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const enterTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    if (transitionKey === prevKeyRef.current) return
    prevKeyRef.current = transitionKey

    setState('exiting')

    exitTimerRef.current = setTimeout(() => {
      exitTimerRef.current = null
      setDisplayChildren(children)
      setState('entering')

      enterTimerRef.current = setTimeout(() => {
        enterTimerRef.current = null
        setState('entered')
      }, 300)
    }, 200)

    return () => {
      if (exitTimerRef.current !== null) {
        clearTimeout(exitTimerRef.current)
        exitTimerRef.current = null
      }
      if (enterTimerRef.current !== null) {
        clearTimeout(enterTimerRef.current)
        enterTimerRef.current = null
      }
    }
  }, [transitionKey, children])

  useEffect(() => {
    return () => {
      if (exitTimerRef.current !== null) {
        clearTimeout(exitTimerRef.current)
        exitTimerRef.current = null
      }
      if (enterTimerRef.current !== null) {
        clearTimeout(enterTimerRef.current)
        enterTimerRef.current = null
      }
    }
  }, [])

  // Keep children in sync when not transitioning (e.g. same-route state change)
  useEffect(() => {
    if (state === 'entered') {
      setDisplayChildren(children)
    }
  }, [children, state])

  return (
    <div className={`page-transition page-transition--${state}`}>
      {displayChildren}
    </div>
  )
}

export default memo(PageTransitionImpl)
