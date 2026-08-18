import { useEffect, useState } from 'react'
import './PageTransition.css'

interface Props {
  children: React.ReactNode
  /** Unique key that triggers the transition (e.g. pathname) */
  transitionKey: string
}

export default function PageTransition({ children, transitionKey }: Props) {
  const [state, setState] = useState<'entering' | 'entered' | 'exiting'>('entered')
  const [displayKey, setDisplayKey] = useState(transitionKey)
  const [displayChildren, setDisplayChildren] = useState(children)

  useEffect(() => {
    if (transitionKey === displayKey) return

    // Phase 1: exit animation
    setState('exiting')

    const exitTimer = setTimeout(() => {
      // Phase 2: swap content while hidden
      setDisplayKey(transitionKey)
      setDisplayChildren(children)
      setState('entering')

      // Phase 3: enter animation completes
      const enterTimer = setTimeout(() => {
        setState('entered')
      }, 300) // matches CSS --page-transition-duration

      return () => clearTimeout(enterTimer)
    }, 200) // exit duration

    return () => clearTimeout(exitTimer)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [transitionKey])

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
