#pragma once

#include "config.hpp"

#include <obs.h>

#include <string>

namespace live_production {

void register_fixture_source();
obs_source_t *create_fixture_source(const FixtureSourceCommand &command,
                                    const PublisherConfig &publisher,
                                    std::string &error);
void release_fixture_source(obs_source_t *source);

}
