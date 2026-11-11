package app.hidlins;

/** Monotonic ownership token for asynchronous local-discovery callbacks. */
final class DiscoveryAttemptTracker {
    private long nextGeneration;
    private long activeGeneration;

    synchronized long begin() {
        nextGeneration += 1;
        if (nextGeneration == 0) nextGeneration += 1;
        activeGeneration = nextGeneration;
        return activeGeneration;
    }

    synchronized boolean isActive(long generation) {
        return generation != 0 && activeGeneration == generation;
    }

    synchronized boolean complete(long generation) {
        if (!isActive(generation)) return false;
        activeGeneration = 0;
        return true;
    }

    synchronized void invalidate() {
        activeGeneration = 0;
    }
}
