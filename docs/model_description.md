<!-- markdownlint-disable MD033 -->
<!-- allow inline html -->
<!-- markdownlint-disable MD028 -->
<!-- allow adjacent block elements -->

# Model Description

## Introduction

### Model Purpose

This Software Requirements Specification (SRS) describes MUSE 2.0 (**M**od**U**lar energy systems
**S**imulation **E**nvironment).
The purpose of MUSE is to provide users with a framework to simulate pathways of energy system
transition, usually in the context of climate change mitigation.

### Model Scope

MUSE is an [Integrated Assessment Modelling](https://unfccc.int/topics/mitigation/workstreams/response-measures/modelling-tools-to-assess-the-impact-of-the-implementation-of-response-measures/integrated-assessment-models-iams-and-energy-environment-economy-e3-models)
framework that is designed to enable users to create and apply an agent-based model that simulates a
market equilibrium on a set of user-defined commodities, over a user-defined time period, for a
user-specified region or set of regions.
MUSE was developed to simulate approaches to climate change mitigation over a long time horizon
(e.g. 5-year steps to 2050 or 2100), but the framework is generalised and can therefore simulate any
market equilibrium.

## Overall Description

### Overview

MUSE 2.0 is the successor to MUSE.
The original MUSE framework is open-source software [available on GitHub](https://github.com/EnergySystemsModellingLab/MUSE_OS),
coded in Python. MUSE 2.0 is implemented following re-design of MUSE to address a range of legacy
issues that are challenging to address via upgrades to the existing MUSE framework, and to implement
the framework in the high-performance Rust language.

MUSE is classified as a recursive dynamic modelling framework in the sense that it iterates on a
single time period to find a market equilibrium, and then moves to the next time period.
Agents in MUSE have limited foresight, reacting only to information available in the current time period.
<!-- markdown-link-check-disable-next-line -->
This is distinct from intertemporal optimisation modelling frameworks (such as [TIMES](https://iea-etsap.org/index.php/etsap-tools/model-generators/times)
and [MESSAGEix](https://docs.messageix.org/en/latest/)) which have perfect foresight over the whole
modelled time horizon.

### Model Concept

MUSE 2.0 is a bottom-up engineering-economic modelling framework that computes a price-induced
supply-demand equilibrium on a set of user-defined commodities.
It does this for each milestone time period within a user-defined time horizon.
This is a "partial equilibrium" in the sense that the framework equilibrates only the user-defined
commodities, as opposed to a whole economy.

MUSE 2.0 is data-driven in the sense that model processing and data are entirely independent, and
user-defined data is at the heart of how the model behaves. It is also "bottom-up" in nature, which
means that it requires users to characterise each individual process that produces or consumes each
commodity, along with a range of other physical, economic and agent parameters.

At a high level, the user defines:

1) The overall temporal arrangements, including the base time period,
    milestone time periods and time horizon, and within-period time
    slice lengths.

2) The service demands for each end-use (e.g. residential heating,
    steel production), for each region, and how that demand is
    distributed between the user-defined time slices within the year.
    Service demands must be given a value for the base time period and
    all milestone time periods in each region.

3) The existing capacity of each process (i.e. assets) in the base time
    period, and the year in which it was commissioned or will be
    decommissioned.

4) The techno-economic attributes (e.g. capital cost, operating costs,
    efficiency, lifetime, input and output commodities, etc) of each
    process. This must include attributes of processes existing in the
    base time period (i.e. assets) and possible future processes that
    could be adopted in future milestone time periods.

5) The agents that choose between technologies by applying search
    spaces, objectives and decision rules. Portions of demand for each
    commodity must be assigned to an agent, and the sum of these
    portions must be one.

The model takes this data, configures and self-checks, and then solves
for a system change pathway:

1) [Initialisation](#1-initialisation)
2) [Base Year Price Discovery](#2-base-year-price-discovery)
3) [Agent Investment](#3-agent-investment)
4) [Carbon Budget Solution (or CO<sub>2</sub> Price Responsiveness)](#4-carbon-budget-solution-or-co2-price-responsiveness)
5) [Find Prices for Next Milestone Year](#5-find-prices-for-next-milestone-year)
6) [Recursively Solve Using Steps (3)-(5) for Each Milestone Year until End](#6-recursively-solve-using-steps-3-5-for-each-milestone-year-until-end)

## Framework Processing Flow

At a high level, the MUSE 2.0 iterative solution concept is as follows:

### 1. Initialisation

Read input data, performing basic temporal set up, commodity and process/asset information.
Consistency check is performed.

### 2. Base Year Price Discovery

Dispatch is executed to determine base year commodity production and consumption.
The result is used to discover commodity prices in the calibrated base year (*t<sub>0</sub>*).

1. Dispatch is solved for all assets and commodities in the system
    simultaneously, where existing assets (known from calibrated
    input data) are operated to meet demand, and to produce/consume
    any intermediate commodities required, and to meet environmental
    constraints if specified.

2. Asset dispatch is merit order based but is subject to
    constraints that represent technical or other limits.

    - For processes, dispatch limits that can be defined are
        minimum, maximum and fixed capacity factors (i.e. percentage
        of capacity) that can be input per time slice, season or
        year.

    - For commodities, user-defined limits can be minimum, maximum
        or fixed total or regional output, input or net production
        by time slice, season or year.

3. Price discovery is implemented via linear programming (cost
    minimisation). The objective function is the cost of operating
    the system over a year, which must be minimised. The decision
    variables are the commodity inputs and outputs of each asset.
    These are constrained by (a) the capacity of the asset and (b)
    the capacity factor limits by time slice/season/year. Energy
    commodity supply/demand must balance for SED (supply equals
    demand) type commodities, and all service demands (SVD
    commodities) must be met. Other commodity production or
    consumption may be subject to constraints (usually annual but
    could be seasonal/diurnal).

4. Based on the resulting dispatch a time sliced price is
    calculated for each commodity using marginal pricing (i.e. the
    operational cost of the most expensive process serving a
    commodity demand). The result of this step is model-generated
    time sliced commodity prices for the base year, *t<sub>0</sub>*.

5. The model then also calculates the prices of commodities that
    are not present in the base year, directly from input data. This
    could be done by calculating the marginal price of the process
    producing the commodity in question with the best objective
    value, where objective values are calculated using the
    utilisation of the next most expensive (marginal cost) asset in
    the dispatch stack, and commodity prices from the price discovery
    at step 3 above.

### 3. Agent Investment

The capacity investment of new assets required in the next milestone year are calculated as follows:

1. **End-of-life capacity decommissioning:** Decommission assets
    that have reached the end of their life in the milestone year.

2. **Agent investment (service demand):** For each service demand,
    for each agent that is responsible for a portion of that demand:

    - For assets, calculate objective value/s assuming the
        utilisation observed from dispatch for that asset in
        step (2) will persist. For assets this calculation does not
        include capital cost as this is sunk cost because the asset
        already exists.

    - For processes, calculate objective value/s assuming the
        utilisation observed from dispatch in step (2) for the asset
        with the marginal cost immediately above the marginal cost
        of this process (and respecting the processes' availability
        constraints). If the process has lower marginal cost than
        any asset, then assume full dispatch (subject to its
        availability constraints). If the process has the same
        marginal cost as an asset, assume the same utilisation as
        that asset. If the process has marginal cost higher than any
        asset, assume zero utilisation.

        > **Note:** Could calculate utilisation
        using time slice level utilisation of asset
        with marginal cost immediately above the process, also
        taking into account capacity factor constraints -- would be
        more accurate in most cases (some complications, e.g. where
        asset/process has conflicting capacity factor
        constraints/utilisation).

    - Add assets/processes to the capacity mix starting with the
          one with the best objective value and keep adding them
          until sufficient capacity exists to meet demand in the
          milestone year. This step must respect process capacity
          constraints (growth, addition and overall limits).

        > **Issue 1:** There is a circularity here. E.g. asset choices
        influence the dispatch of other assets, which in turn can
        influence objective values, which in turn can influence
        asset choices. An incomplete solution is to run dispatch
        again, update utilisations of assets and proposed new
        assets, and repeat step 3.2 again, to see if
        any asset's objective has deteriorated to the point where it
        can be replaced, and keep going around this loop until
        nothing changes between loops - but there will certainly be
        cases where this does not converge - what to do?

        > **Issue 2:** Also, commodity prices influence dispatch (and thus
        objective value), so upstream decisions also impact outcome
        here. We're not worrying about this as it's a deliberate
        feature of MUSE - investors are assuming observed prices
        persist.

3. **Agent investment (commodities):** For each commodity demanded,
    starting with those commodities consumed by the end-use assets
    (i.e. those assets that output a service demanded), calculate
    the capacity investments required to serve these commodity
    demands:

    - Run dispatch to determine final commodity demand related to
        all end use technologies. Determine production capacity
        required (output in each time slice) to serve this demand,
        for each commodity.

    - Follows step 3.2 above to determine capacity
        mix for each commodity.

    - Continue this process, moving further upstream, until there
          are no commodity demands left to serve.

        > **Issue 3:** Circularities here, e.g. power system capacity is
        required to produce H2, but also H2 can be consumed in the
        power sector so H2 capacity is needed to produce it, which
        in turn requires more power system capacity. Could check if
        demand for each commodity has changed at the end of a run
        through all commodities, and if it has then run the capacity
        investment algorithm again for that commodity.

        > **Issue 4:** What about commodities that are consumed but not
        produced, or produced but not consumed? Do this capacity
        investment step only for SED commodities? And also check for
        processes that consume non-balance commodities, and check if
        they can make money - invest in them if they do - requires
        specific objective of NPV.

4. **Decision-rule-based capacity decommissioning:** Decommission
    assets that were not selected in steps 3.2-3.3. This could
    happen when, for example, carbon prices are high and emitting
    assets become unfavorable as a result (e.g. operating them is
    too expensive and cannot compete with new technology even though
    the latter has capital cost included).

### 4. Carbon budget solution (or CO<sub>2</sub> price responsiveness)

Steps (2)-(3) are initially run with the CO<sub>2</sub> price from the previous milestone year.
After completion, run dispatch with a CO<sub>2</sub> budget equal to the user prescribed level
(if it exists), and record the resulting CO<sub>2</sub> price (dual solution of the CO<sub>2</sub> constraint).
If CO<sub>2</sub> price is less than zero then re-run dispatch without the budget constraint
and set CO<sub>2</sub> price to zero.
**Alternatively,** a user might specify a CO<sub>2</sub> price for the
whole time horizon, and no carbon budget, in which case the model
runs dispatch with the specified carbon price relating to each
milestone year in steps (2)-(3) and no further processing is needed
here.

> **Issue 5:**  If there
is no solution, then budget cannot be met (warn the user), and
re-run dispatch without the budget constraint.
In this case, what should the CO<sub>2</sub> price be set to?

### 5. Find Prices for next Milestone Year

Using the CO<sub>2</sub> price from
step (4), run dispatch again (note this is not needed if running
dispatch with the CO<sub>2</sub> budget found a solution, because that will be
the correct system dispatch). This determines the prices and final
commodity consumption and production for the present milestone year,
record results. Use these prices and perform steps (2) and (3) above
for the next milestone year, alongside calculated prices for any
commodities not present in the system (as per step 2.5).

### 6. Recursively Solve Using Steps (3)-(5) for Each Milestone Year until End

The model then moves to the next milestone time period
and repeats the process, beginning with prices from the last-solved
time period. This process continues until the end of the time
horizon is reached.
