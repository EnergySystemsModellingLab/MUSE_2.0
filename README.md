<!-- markdownlint-disable MD041 -->
[![Build and test](https://github.com/EnergySystemsModellingLab/MUSE_2.0/actions/workflows/cargo-test.yml/badge.svg)](https://github.com/EnergySystemsModellingLab/MUSE_2.0/actions/workflows/cargo-test.yml)
[![codecov](https://codecov.io/github/EnergySystemsModellingLab/MUSE_2.0/graph/badge.svg?token=nV8gp1NCh8)](https://codecov.io/github/EnergySystemsModellingLab/MUSE_2.0)
[![GitHub](https://img.shields.io/github/license/EnergySystemsModellingLab/MUSE_2.0)](https://raw.githubusercontent.com/EnergySystemsModellingLab/MUSE_2.0/main/LICENSE)

# MUSE 2.0

MUSE 2.0 (**M**od**U**lar energy systems **S**imulation **E**nvironment) is a tool for running
simulations of energy systems, written in Rust. Its purpose is to provide users with a framework to
simulate pathways of energy system transition, usually in the context of climate change mitigation.

It is the successor to [MUSE], which is written in Python. It was developed following re-design of
MUSE to address a range of legacy issues that are challenging to address via upgrades to the
existing MUSE framework, and to implement the framework in the high-performance Rust language.

:construction: **Please note that this code is under heavy development and is not yet suitable for
end users. Watch this space!** :construction:

## Model Overview

MUSE is an [Integrated Assessment Modelling] framework that is designed to enable users to create
and apply an agent-based model to simulate a market equilibrium on a set of user-defined
commodities, over a user-defined time period, for a user-specified region or set of regions. MUSE
was developed to simulate approaches to climate change mitigation over a long time horizon (e.g.
5-year steps to 2050 or 2100), but the framework is generalised and can therefore simulate any
market equilibrium.

It is a recursive dynamic modelling framework in the sense that it iterates on a single time period
to find a market equilibrium, and then moves to the next time period. Agents in MUSE have limited
foresight, reacting only to information available in the current time period. This is distinct from
intertemporal optimisation modelling frameworks (such as [TIMES] and [MESSAGEix]) which have perfect
foresight over the whole modelled time horizon.

[MUSE]: https://github.com/EnergySystemsModellingLab/MUSE_OS
[Integrated Assessment Modelling]: https://unfccc.int/topics/mitigation/workstreams/response-measures/modelling-tools-to-assess-the-impact-of-the-implementation-of-response-measures/integrated-assessment-models-iams-and-energy-environment-economy-e3-models
[TIMES]: https://iea-etsap.org/index.php/etsap-tools/model-generators/times
[MESSAGEix]: https://docs.messageix.org/en/latest

## Getting started

To start using MUSE 2.0, please refer to [the documentation]. If you wish to develop MUSE 2.0 or
build it from source, please see [the developer guide].

[the documentation]: https://energysystemsmodellinglab.github.io/MUSE_2.0/introduction.html
[the developer guide]: https://energysystemsmodellinglab.github.io/MUSE_2.0/developer_guide.html

## Copyright

Copyright © 2025 Imperial College London
