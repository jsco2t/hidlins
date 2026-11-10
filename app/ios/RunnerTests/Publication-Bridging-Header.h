#ifndef HIDLINS_PUBLICATION_TEST_BRIDGING_HEADER_H
#define HIDLINS_PUBLICATION_TEST_BRIDGING_HEADER_H

// Import the app boundary explicitly instead of relying on Swift's deprecated
// implicit bridging-header import through @testable import Runner.
#include "../Runner/Runner-Bridging-Header.h"

#if defined(HIDLINS_IOS_PUBLICATION_FIXTURE)
// Only referenced by the explicitly configured simulator test target.
int32_t hidlins_ios_publication_fixture_create(
    const uint8_t *root, size_t root_length, uint8_t *ticket, size_t capacity);
int32_t hidlins_ios_publication_fixture_lock(void);
uint32_t hidlins_ios_publication_fixture_status(void);
int32_t hidlins_ios_publication_fixture_finish(void);
#endif

#endif
