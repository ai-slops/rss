#pragma once

#include <memory>

namespace live_production {

class PublisherTransportFault final {
 public:
  PublisherTransportFault();
  ~PublisherTransportFault();

  PublisherTransportFault(const PublisherTransportFault &) = delete;
  PublisherTransportFault &operator=(const PublisherTransportFault &) = delete;
  PublisherTransportFault(PublisherTransportFault &&) = delete;
  PublisherTransportFault &operator=(PublisherTransportFault &&) = delete;

  bool freeze();
  bool resume();
  bool frozen() const;

 private:
  class Impl;
  std::unique_ptr<Impl> impl_;
};

}
