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

  useEffect(() => {
    if (transitionKey === prevKeyRef.current) return
    prevKeyRef.current = transitionKey

    setState('exiting')

    const exitTimer = setTimeout(() => {
      setDisplayChildren(children)
      setState('entering')

      const enterTimer = setTimeout(() => {
        setState('entered')
      }, 300)

      return () => clearTimeout(enterTimer)
    }, 200)

    return () => clearTimeout(exitTimer)
  }, [transitionKey, children])

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
