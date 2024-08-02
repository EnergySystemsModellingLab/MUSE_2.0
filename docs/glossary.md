# Glossary

**Activity:** The input/s or output/s of a process that are limited by its capacity. For example, a
500MW power station can output 500MW of electrical power, or a 50MW electrolyser consumes up to 50MW
of electrical power to produce hydrogen. The *Primary Activity Commodity* specifies which output/s
or input/s are linked to the process capacity.

**Agent:** A decision-making entity in the system. An *Agent* is responsible for serving a
user-specified portion of a *Commodity* demand or *Service Demand*. *Agents* invest in and operate
*Assets* to serve demands.

**Agent Objective/s:** One or more objectives that an *Agent* considers when deciding which
technology to invest in to serve demand. Objectives can be economic, environmental, or others.

**Asset:** Once an *Agent* makes an investment, the related capacity of their chosen technology
becomes an *Asset* that they own and operate. An *Asset* is an instance of a technology, it has a
specific capacity, and a decommissioning year. A set of *Assets* exist in the base year sufficient
to serve base year demands (i.e. a calibrated base year).

**Base Year:** The starting year of a model run. The base year is typically calibrated to known
data, including technology stock and commodity consumption/production.

**Calibration:** The process of ensuring that the model represents the system being modelled in a
historical base year.

**Capacity:** The maximum output (or input) of an *Asset*, as measured by units of the *Primary
Activity Commodity*.

**Capacity Factor:** The percentage of maximum output (or input) that the *Asset* delivers over a
specified time period. The time period could be a single time slice, a season, or a whole year.

**Capital Cost:** The overnight capital cost of a process, measured in units of the *Primary
Activity Commodity* divided by CAP2ACT. CAP2ACT is a factor that converts 1 unit of capacity to
maximum activity of the primary activity commodity per year. For example, if capacity is measured in
GW and activity is measured in PJ, CAP2ACT for the process is 31.536 because 1 GW of capacity can
produce 31.536 PJ energy output in a year.

<!-- markdownlint-disable-next-line MD033 -->
**Commodity:** A substance (e.g. CO<sub>2</sub>) or form of energy (e.g. electricity) that can be
produced and/or consumed by technologies in the model. A *Service Demand* is a type of commodity
that is defined at the end point of the system.

**Decision Rule:** The rule via which an *Agent* uses the *Objective/s* to decide between technology
options to invest in. Examples include single objective, weighted sum between multiple objectives,
or epsilon constraint where a secondary objective is considered if two options with similar primary
objectives are identified.

**Dispatch:** The way in which *Assets* are operated to serve demand. MUSE 2.0 uses merit order
dispatch, subject to capacity factor constraints that can be defined by the user.

**End Year:** The final year in the model time horizon.

**Equivalent Annual Cost (EAC):** An *Agent* objective, representing the cost of serving all or part
of an *Agent’s* demand for a year, considering the *Asset’s* entire lifetime.

**Fixed Operating Cost:** The annual operating cost charged per unit of capacity.

**Input Commodity/ies:** The commodities that flow into a process.

**Levelized Cost of X (LCOX):** An *Agent* objective, representing the expected cost of 1 unit of
output commodity X from a process over its lifetime under a specified discount rate.

**Lifetime:** The lifetime of a process, measured in years.

**Milestone Years:** A set of years in the model time horizon where outputs are recorded. For
example, with a 2025 Base Year and Time Horizon to 2100, a user might choose to record outputs in
5-year steps.

**Merit Order:** A method of operating *Assets* when the cheapest is dispatched first, followed by
the next most expensive, etc, until demand is served. Also called “unit commitment.”

**Output Commodity/ies:** The commodities that flow out of a process.

**Primary Activity Commodity (PAC):** The PACs specify which output/s are linked to the process
capacity. The combined output of all PACs cannot exceed the *Asset’s* capacity. A user can define
which output/s are PACs.

**Process/Technology:** An available process that converts input commodities to output commodities.
Processes have economic attributes of capital cost, fixed operating cost per unit capacity, non-fuel
variable operating cost per unit activity, and technology risk discount rate. They have physical
attributes of quantity and type of input and output commodities (which implicitly specify
efficiency), capacity factor limits (by time slice, season and/or year), lifetime (years).

**Region:** A geographical area that is modelled. Regions primarily determine trade boundaries.

**Season:** A year is usually broken down into seasons in the model. For example, summer, winter,
other.

**Sector:** Models are often broken down into sectors, each of which is associated with specific
*Service Demands* or specific *Commodity* production. For example, the residential sector, the power
sector, etc.

**Service Demand:** A Service Demand is a type of commodity that is consumed at the boundary of the
modelled system. For example, tonne-kilometers of road freight, PJ of useful heat demand, etc.

**Technology Discount Rate:** The discount rate used to calculate any technology-specific agent
economic objectives that require a discount rate. For example, Equivalent Annual Cost, Net Present
Value, Levelized Cost of X, etc.

**Time Horizon:** The overall period modelled. For example, 2025&ndash;2100.

**Time Period:** Refers to a specific year in the time horizon.

**Time Slice:** The finest time period in the model. The maximum time slice length is 1 year (where
a model does not represent seasons or within-day (diurnal) variation). A typical model will have
several diurnal time slices, and several seasonal time slices.

**Variable Operating Cost:** The variable operating cost charged per unit of input or output of the
*Primary Activity Commodity* of the process.